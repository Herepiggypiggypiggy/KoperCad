//! The three exports of host API 1 and the buffers behind them.
//!
//! No `unsafe` block anywhere: the host writes the request into a buffer this module owns, and
//! `kc_alloc` hands out that buffer's address as a number. The only exception to the crate's
//! `deny(unsafe_code)` is the export attribute itself (`#[unsafe(no_mangle)]` is the Rust 2024
//! spelling of "export this name"); it carries no memory access.

use std::cell::RefCell;

use crate::Engine;

struct Buffers {
    engine: Engine,
    request: Vec<u8>,
    reply: Vec<u8>,
}

thread_local! {
    static STATE: RefCell<Buffers> = RefCell::new(Buffers {
        engine: Engine::new(),
        request: Vec::new(),
        reply: Vec::new(),
    });
}

/// A buffer of `len` bytes for the host to write a request into; returns its address.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn kc_alloc(len: i32) -> i32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.request = vec![0; usize::try_from(len).unwrap_or(0)];
        s.request.as_ptr() as usize as i32
    })
}

/// Handles the request the host wrote (`ptr` is the address `kc_alloc` returned). Returns
/// `(reply_ptr << 32) | reply_len`.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn kc_call(_ptr: i32, len: i32) -> i64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let len = usize::try_from(len).unwrap_or(0).min(s.request.len());
        let request = String::from_utf8_lossy(&s.request[..len]).into_owned();
        let reply = s.engine.handle(&request);
        s.reply = reply.into_bytes();
        let ptr = s.reply.as_ptr() as usize as u64;
        ((ptr << 32) | s.reply.len() as u64) as i64
    })
}

/// The host is done with a buffer (the request or the reply).
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn kc_free(_ptr: i32, _len: i32) {}
