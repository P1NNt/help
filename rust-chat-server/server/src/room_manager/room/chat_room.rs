use comms::event::{self, Event, UserMessageBroadcastEvent};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use std::collections::VecDeque;

use super::{
    user_registry::UserRegistry,
    user_session_handle::UserSessionHandle,
    SessionAndUserId,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
/// [ChatRoomMetadata] holds the metadata that identifies a chat room
pub struct ChatRoomMetadata {
    pub name: String,
    pub description: String,
}

const BROADCAST_CHANNEL_CAPACITY: usize = 100;
const MAX_HISTORY: usize = 10;

#[derive(Debug)]
/// [ChatRoom] handles the participants of a chat room, broadcasts events,
/// and maintains a capped message history.
pub struct ChatRoom {
    metadata: ChatRoomMetadata,
    broadcast_tx: broadcast::Sender<Event>,
    user_registry: UserRegistry,
    history: VecDeque<UserMessageBroadcastEvent>,
}

impl ChatRoom {
    /// Create a new ChatRoom with empty participant list and history
    pub fn new(metadata: ChatRoomMetadata) -> Self {
        let (broadcast_tx, _) = broadcast::channel(BROADCAST_CHANNEL_CAPACITY);
        ChatRoom {
            metadata,
            broadcast_tx,
            user_registry: UserRegistry::new(),
            history: VecDeque::with_capacity(MAX_HISTORY),
        }
    }

    /// Retrieve unique user IDs currently in the room
    pub fn get_unique_user_ids(&self) -> Vec<String> {
        self.user_registry.get_unique_user_ids()
    }

    /// Send a new chat message: records it in history (evicting oldest if needed)
    /// and broadcasts to all subscribers.
    pub fn send_message(&mut self, session_and_user_id: &SessionAndUserId, content: String) {
        let msg = UserMessageBroadcastEvent {
            room: self.metadata.name.clone(),
            user_id: session_and_user_id.user_id.clone(),
            content,
        };
        if self.history.len() == MAX_HISTORY {
            self.history.pop_front();
        }
        self.history.push_back(msg.clone());
        let _ = self.broadcast_tx.send(Event::UserMessage(msg));
    }

    /// Return a snapshot of stored messages
    pub fn get_history(&self) -> Vec<UserMessageBroadcastEvent> {
        self.history.iter().cloned().collect()
    }

    /// Add a participant to the room and broadcast that they joined
    /// Returns a receiver for live events and a session handle
    pub fn join(
        &mut self,
        session_and_user_id: &SessionAndUserId,
    ) -> (broadcast::Receiver<Event>, UserSessionHandle) {
        let broadcast_tx = self.broadcast_tx.clone();
        let broadcast_rx = broadcast_tx.subscribe();
        let handle = UserSessionHandle::new(
            self.metadata.name.clone(),
            broadcast_tx.clone(),
            session_and_user_id.clone(),
        );

        if self.user_registry.insert(&handle) {
            let evt = Event::RoomParticipation(
                event::RoomParticipationBroadcastEvent {
                    user_id: session_and_user_id.user_id.clone(),
                    room: self.metadata.name.clone(),
                    status: event::RoomParticipationStatus::Joined,
                },
            );
            let _ = self.broadcast_tx.send(evt);
        }

        (broadcast_rx, handle)
    }

    /// Remove a participant and broadcast that they left
    pub fn leave(&mut self, handle: UserSessionHandle) {
        if self.user_registry.remove(&handle) {
            let evt = Event::RoomParticipation(
                event::RoomParticipationBroadcastEvent {
                    user_id: handle.user_id().to_string(),
                    room: self.metadata.name.clone(),
                    status: event::RoomParticipationStatus::Left,
                },
            );
            let _ = self.broadcast_tx.send(evt);
        }
    }
}

