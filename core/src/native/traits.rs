// Copyright (C) 2019  Pierre Krieger
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use alloc::vec::Vec;
use core::future::Future;
use redshirt_syscalls_interface::{EncodedMessage, MessageId, Pid};

/// Reference to a native program.
///
/// This trait is not meant to be implemented on types `T`. Instead, if `T` represents the state
/// of the native program, then the [`NativeProgramRef`] trait should be implemented on `&'a T`
/// instead (where `'a` is the lifetime parameter of the trait). This design is necessary due to
/// the lack of HRTBs in the Rust language.
pub trait NativeProgramRef<'a>: Clone {
    /// Future resolving to the next event the [`NativeProgram`] emits.
    ///
    /// Typically set to `Pin<Box<dyn Future<Output = NativeProgramEvent<Self::MessageIdWrite>> + Send + 'a>>`.
    type Future: Future<Output = NativeProgramEvent<Self::MessageIdWrite>> + Send + 'a;
    /// When the [`NativeProgram`] emits a message, this item is used by the caller to notify of
    /// the [`MessageId`] that has been emitted.
    type MessageIdWrite: NativeProgramMessageIdWrite;

    /// Returns a `Future` resolving to when the [`NativeProgram`] wants to do something.
    fn next_event(self) -> Self::Future;

    /// Notify the [`NativeProgram`] that a message has arrived on one of the interface that it
    /// has registered.
    fn interface_message(
        self,
        interface: [u8; 32],
        message_id: Option<MessageId>,
        emitter_pid: Pid,
        message: EncodedMessage,
    );

    /// Notify the [`NativeProgram`] that the program with the given [`Pid`] has terminated.
    fn process_destroyed(self, pid: Pid);

    /// Notify the [`NativeProgram`] of a response to a message that it has previously emitted.
    fn message_response(self, message_id: MessageId, response: Result<EncodedMessage, ()>);
}

/// Event generated by a [`NativeProgram`].
pub enum NativeProgramEvent<TMsgIdWrite> {
    /// Request to emit a message.
    ///
    /// If the interface is not available, the message will be buffered.
    Emit {
        /// Interface to emit the message on.
        interface: [u8; 32],
        /// If we expect an answer, contains an object that allows indicating to the
        /// [`NativeProgramRef`] which `MessageId` has been attributed.
        ///
        /// `None` if the [`NativeProgramRef`] doesn't expect an answer for this message.
        message_id_write: Option<TMsgIdWrite>,
        /// Message to send.
        message: EncodedMessage,
    },
    /// Request to cancel a previously-emitted message.
    CancelMessage { message_id: MessageId },
    /// Answer a message previously received with [`NativeProgramRef::interface_message`].
    Answer {
        /// Message to answer.
        message_id: MessageId,
        /// Answer to the message. Can be an error if the message is invalid.
        answer: Result<EncodedMessage, ()>,
    },
}

/// Trait used to write back the [`MessageId`] when the program emits a message.
pub trait NativeProgramMessageIdWrite {
    /// Write the [`MessageId`] of the emitted message.
    fn acknowledge(self, message_id: MessageId);
}

/// Dummy implementation of [`NativeProgramMessageIdWrite`] that does nothing.
// TODO: implement trait on `!` instead, when stable
#[derive(Debug, Default)]
pub struct DummyMessageIdWrite;

impl NativeProgramMessageIdWrite for DummyMessageIdWrite {
    fn acknowledge(self, _: MessageId) {}
}
