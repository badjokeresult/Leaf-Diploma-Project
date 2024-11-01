use std::net::SocketAddr;
use std::ffi::CStr;
use std::os::raw::{c_char, c_ulong, c_void, c_ushort};

use tokio::sync::mpsc::{channel, Receiver};
use tokio::runtime::Builder;
use tokio::task::{spawn, JoinHandle};

mod codec;
mod crypto;
mod hash;
mod message;
mod server;
mod shared_secret;
mod storage;

use message::*;
use server::*;
use shared_secret::*;
use crypto::*;
use hash::*;

pub mod consts {
    pub const WORKING_FOLDER_NAME: &str = ".leaf";
    pub const PASSWORD_FILE_NAME: &str = "passwd.txt";
    pub const GAMMA_FILE_NAME: &str = "gamma.bin";
    pub const SENDING_REQUEST_TYPE: u8 = 0;
    pub const SENDING_ACKNOWLEDGEMENT_TYPE: u8 = 1;
    pub const RETRIEVING_REQUEST_TYPE: u8 = 2;
    pub const RETRIEVING_ACKNOWLEDGEMENT_TYPE: u8 = 3;
    pub const CONTENT_FILLED_TYPE: u8 = 4;
    pub const EMPTY_TYPE: u8 = 5;
    pub const MAX_MESSAGE_SIZE: usize = 65243;
    pub const MAX_DATAGRAM_SIZE: usize = 65507;
    pub const DEFAULT_CHUNKS_STOR_FOLDER: &str = "chunks";
}

#[repr(C)]
pub struct CVec {
    len: c_ulong,
    capacity: c_ulong,
    data: *mut c_void,
}

#[repr(C)]
pub struct InitializeParams {
    receiver: *mut c_void,
    server: *const c_void,
    handles_vec: CVec,
}

#[no_mangle]
pub extern "C" fn init(addr: *const c_char, broadcast_addr: *const c_char, num_threads: c_ulong) -> InitializeParams {
    let builder = Builder::new_multi_thread()
        .enable_all()
        .build().unwrap();

    let addr = unsafe { CStr::from_ptr(addr).to_str().unwrap() };
    let broadcast_addr = unsafe { CStr::from_ptr(broadcast_addr).to_str().unwrap() };
    let (tx, rx) = channel::<(Message, SocketAddr)>(1024);
    let server = builder.block_on(async { BroadcastUdpServer::new(addr, broadcast_addr, tx.clone()).await });

    let mut handles = vec![];
    for _ in 0..num_threads {
        let server = server.clone();
        handles.push(builder.block_on(async { spawn(async move {
            server.listen().await;
        }) }));
    };

    let rx_clone = Box::new(rx);
    let rx_ptr = Box::into_raw(rx_clone) as *mut c_void;
    let server_clone = Box::new(server);
    let server_ptr = Box::into_raw(server_clone) as *const c_void;
    let (len, capacity) = (handles.len() as c_ulong, handles.capacity() as c_ulong);
    let handles_boxed_slice = Vec::into_boxed_slice(handles);
    let handles_ptr = Box::into_raw(handles_boxed_slice) as *mut c_void;

    InitializeParams {
        receiver: rx_ptr,
        server: server_ptr,
        handles_vec: CVec {
            len,
            capacity,
            data: handles_ptr,
        }
    }
}

#[no_mangle]
pub extern "C" fn send_file(len: c_ulong, capacity: c_ulong, content: *mut c_ushort, server: *const c_void, receiver: *mut c_void) -> CVec {
    let content: Vec<u8> = unsafe { Vec::from_raw_parts(content as *mut u8, len as usize, capacity as usize) };
    let server = unsafe { Box::from_raw(server as *mut BroadcastUdpServer) };
    let mut receiver = unsafe { Box::from_raw(receiver as *mut Receiver<(Message, SocketAddr)>) };
    let builder = Builder::new_multi_thread()
        .enable_all()
        .build().unwrap();

    let sharer = ReedSolomonSecretSharer::new();
    let chunks = sharer.split_into_chunks(&content).unwrap();

    let encryptor = KuznechikEncryptor::new().unwrap();
    let mut encrypted_chunks = vec![];
    for chunk in chunks {
        if let Some(c) = chunk {
            encrypted_chunks.push(Some(builder.block_on(async { encryptor.encrypt_chunk(&c).await.unwrap() })));
        } else {
            encrypted_chunks.push(None);
        }
    }

    let hasher = StreebogHasher::new();
    let mut hashes = vec![];
    for chunk in &encrypted_chunks {
        if let Some(c) = chunk {
            hashes.push(Some(hasher.calc_hash_for_chunk(c)));
        } else {
            hashes.push(None);
        }
    }

    let mid = encrypted_chunks.len() / 2;

    for i in 0..mid {
        let (chunk, hash) = match encrypted_chunks.get(i).unwrap() {
            Some(c) => match hashes.get(i).unwrap() {
                Some(h) => (c, h),
                None => panic!(),
            },
            None => match encrypted_chunks.get(mid + i).unwrap() {
                Some(c) => match hashes.get(mid + i).unwrap() {
                    Some(h) => (c, h),
                    None => panic!(),
                },
                None => panic!(),
            }
        };
        builder.block_on(async { server.send_chunk(hash, chunk, &mut receiver).await }).unwrap();
    }

    let len = hashes.len();
    let capacity = hashes.capacity();
    let hashes_ptr = Box::into_raw(Vec::into_boxed_slice(hashes)) as *mut c_void;

    CVec {
        len: len as c_ulong,
        capacity: capacity as c_ulong,
        data: hashes_ptr,
    }
}

#[no_mangle]
pub extern "C" fn recv_content(len: c_ushort, capacity: c_ushort, hashes: *mut c_void, server: *const c_void, receiver: *mut c_void) -> CVec {
    let server = unsafe { Box::from_raw(server as *mut BroadcastUdpServer) };
    let mut receiver= unsafe { Box::from_raw(receiver as *mut Receiver<(Message, SocketAddr)>) };
    let hashes: Vec<Option<Vec<u8>>> = unsafe { Vec::from_raw_parts(hashes as *mut Option<Vec<u8>>, len as usize, capacity as usize) };
    let builder = Builder::new_multi_thread()
        .enable_all()
        .build().unwrap();

    let mut chunks = vec![];
    for hash in hashes {
        if let Some(c) = hash {
            chunks.push(Some(builder.block_on(async { server.recv_chunk(&c, &mut receiver).await.unwrap() })));
        } else {
            chunks.push(None);
        }
    }

    let decryptor = KuznechikEncryptor::new().unwrap();
    let mut decrypted_chunks = vec![];
    for chunk in chunks {
        if let Some(c) = chunk {
            decrypted_chunks.push(Some(builder.block_on(async { decryptor.decrypt_chunk(&c).await.unwrap() })));
        } else {
            decrypted_chunks.push(None);
        }
    }

    let sharer = ReedSolomonSecretSharer::new();
    let content = sharer.recover_from_chunks(decrypted_chunks).unwrap();

    let len = content.len();
    let capacity = content.capacity();
    let content_ptr = Box::into_raw(Vec::into_boxed_slice(content)) as *mut c_void;

    CVec {
        len: len as c_ulong,
        capacity: capacity as c_ulong,
        data: content_ptr
    }
}

#[no_mangle]
pub extern "C" fn shutdown(len: c_ulong, capacity: c_ulong, handles: *mut c_void, server: *const c_void, receiver: *mut c_void) {
    let builder = Builder::new_multi_thread()
        .enable_all()
        .build().unwrap();

    let handles = unsafe { Vec::from_raw_parts(handles as *mut JoinHandle<()>, len as usize, capacity as usize) };
    for handle in handles {
        handle.abort();
    }
    let server = unsafe { Box::from_raw(server as *mut BroadcastUdpServer) };
    builder.block_on(async { server.shutdown().await; });

    let receiver = unsafe { Box::from_raw(receiver as *mut Receiver<(Message, SocketAddr)>) };
    drop(receiver);
}