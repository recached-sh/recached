samples: 22612, user cycles/op 4249

### Stack contains (inclusive)

| share | cycles/op | label |
|---:|---:|---|
|   7.4%    312.7 | `frame build (broadcast_for_at, all)` |
|   0.6%     25.6 | `frame build: allocator` |
|   0.0%      0.0 | `frame build: fmt` |
|   1.0%     42.7 | `owned command (to_command)` |
|   6.8%    290.2 | `stripe locks` |
|   7.3%    309.5 | `on_write_logged (all)` |
|   0.0%      0.0 | `append_client_write (queue + ticket)` |
|   2.1%     87.6 | `gate poll_settled (all)` |
|   0.5%     22.9 | `AOF lead/perform (all)` |
|   0.0%      0.0 | `AOF write(2) wrapper` |
|   0.0%      0.0 | `yield_now` |
|   7.4%    314.0 | `clock reads` |
|   0.0%      0.0 | `now_unix_ms` |
|   6.1%    259.3 | `metrics record` |
|  40.8%   1733.2 | `store execute (all)` |
|   0.7%     29.1 | `reply serialize` |
|   7.5%    317.0 | `decode` |
|   2.7%    113.7 | `allocator (all)` |
|   0.0%      0.0 | `socket send/recv wrappers` |
|   2.6%    110.5 | `BufWriter flush` |
|   0.0%      0.0 | `Value drop` |
|   0.0%      0.0 | `Command drop` |

### Breakdown: innermost context x leaf class (each sample once)

| context | leaf class | share | cycles/op |
|---|---|---:|---:|
| store execute | code |  23.7% |    1005 |
| store execute | libc (memcpy, syscall wrappers) |  10.8% |     457 |
| store execute | allocator |   0.2% |       7 |
| **store execute** | **all** | ** 34.6%** | **   1469** |
| connection (decode, reply, socket) | code |  19.4% |     824 |
| connection (decode, reply, socket) | libc (memcpy, syscall wrappers) |   4.7% |     200 |
| connection (decode, reply, socket) | allocator |   0.9% |      37 |
| connection (decode, reply, socket) | tokio mpsc |   0.6% |      24 |
| connection (decode, reply, socket) | tokio wake/schedule |   0.2% |       7 |
| **connection (decode, reply, socket)** | **all** | ** 25.7%** | **   1091** |
| ordered write path (rest) | code |   9.0% |     384 |
| ordered write path (rest) | tokio mpsc |   1.5% |      63 |
| ordered write path (rest) | libc (memcpy, syscall wrappers) |   1.4% |      58 |
| ordered write path (rest) | allocator |   0.2% |       8 |
| **ordered write path (rest)** | **all** | ** 12.1%** | **    513** |
| frame build (broadcast_for_at) | code |   5.4% |     231 |
| frame build (broadcast_for_at) | libc (memcpy, syscall wrappers) |   1.3% |      57 |
| frame build (broadcast_for_at) | allocator |   0.6% |      26 |
| **frame build (broadcast_for_at)** | **all** | **  7.4%** | **    313** |
| on_write_logged (frame copy, dispatch) | code |   6.1% |     257 |
| on_write_logged (frame copy, dispatch) | allocator |   0.7% |      31 |
| on_write_logged (frame copy, dispatch) | libc (memcpy, syscall wrappers) |   0.5% |      21 |
| **on_write_logged (frame copy, dispatch)** | **all** | **  7.3%** | **    310** |
| stripe locks | code |   5.5% |     234 |
| stripe locks | tokio mpsc |   1.3% |      56 |
| **stripe locks** | **all** | **  6.8%** | **    290** |
| AOF append/lead/gate | tokio io/split |   1.4% |      61 |
| AOF append/lead/gate | code |   0.7% |      31 |
| AOF append/lead/gate | tokio wake/schedule |   0.4% |      17 |
| AOF append/lead/gate | libc (memcpy, syscall wrappers) |   0.4% |      16 |
| AOF append/lead/gate | allocator |   0.2% |       9 |
| AOF append/lead/gate | atomics/Arc drop |   0.0% |       1 |
| **AOF append/lead/gate** | **all** | **  3.2%** | **    135** |
| tokio scheduler | tokio wake/schedule |   1.9% |      81 |
| tokio scheduler | code |   0.8% |      32 |
| tokio scheduler | tokio io/split |   0.2% |       7 |
| tokio scheduler | libc (memcpy, syscall wrappers) |   0.0% |       2 |
| tokio scheduler | tokio mpsc |   0.0% |       0 |
| tokio scheduler | allocator |   0.0% |       0 |
| **tokio scheduler** | **all** | **  2.9%** | **    123** |
| tokio io driver | code |   0.1% |       4 |
| tokio io driver | libc (memcpy, syscall wrappers) |   0.0% |       1 |
| **tokio io driver** | **all** | **  0.1%** | **      5** |

### Self (leaf)

| share | cycles/op | function |
|---:|---:|---|
|  11.2%    477.7 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   7.3%    308.2 | `libc.so.6:+1c7919` |
|   6.0%    252.9 | `<recached_server::server_state::ServerState>::on_write_logged::{closure#0}` |
|   4.7%    201.5 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|   4.3%    182.1 | `<core_engine::request::RequestDecoder>::decode_any` |
|   3.8%    162.9 | `[vdso]:+a8c` |
|   3.0%    127.6 | `recached_server::propagation::execute_ordered_write::{closure#0}` |
|   2.9%    121.4 | `recached_server::clients::record_metrics` |
|   2.2%     94.9 | `<core_engine::store::KeyValueStore>::set::<&str, &[u8]>` |
|   2.2%     93.0 | `<recached_server::replication::ReplHub>::lock_stripes::<core::iter::sources::empty::Empty<&str>>::{closure#0}` |
|   2.1%     89.3 | `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` |
|   2.0%     87.0 | `recached_server::propagation::resp_push` |
|   1.7%     74.2 | `core_engine::resp::write_header` |
|   1.5%     63.5 | `recached_server::propagation::apply_write_effects_identified::{closure#0}` |
|   1.5%     63.1 | `<core_engine::store::KeyValueStore>::execute_reporting_ref` |
|   1.5%     62.6 | `<tokio::sync::batch_semaphore::Semaphore>::add_permits_locked` |
|   1.5%     62.2 | `core::str::converts::from_utf8` |
|   1.4%     57.9 | `recached_server::propagation::try_execute_borrowed` |
|   1.3%     53.4 | `recached_server::propagation::execute_locked_write_identified::{closure#0}` |
|   1.2%     52.6 | `<tokio::sync::batch_semaphore::Acquire as core::future::future::Future>::poll` |
|   1.2%     49.6 | `<core_engine::store::KeyValueStore>::run_probed_write_ordered::<&core_engine::cmd::Command, <core_engine::sto…` |
|   1.1%     48.1 | `core_engine::store::entry_size` |
|   1.1%     47.9 | `_rjem_malloc` |
|   1.1%     45.9 | `core_engine::resp::format_i64` |
|   1.1%     45.1 | `core::ptr::drop_glue::<smallvec::SmallVec<[tokio::sync::mutex::MutexGuard<()>; 2]>>` |
|   1.1%     44.7 | `<tokio::sync::mutex::Mutex<std::collections::hash::map::HashMap<alloc::string::String, alloc::vec::Vec<recach…` |
|   1.0%     42.8 | `core_engine::request::build_ref` |
|   1.0%     41.3 | `<core_engine::store::SlotRef as core::ops::drop::Drop>::drop` |
|   0.9%     39.3 | `core_engine::store::lock_stripe` |
|   0.9%     37.0 | `<std::sys::pal::unix::time::Timespec>::sub_timespec` |
|   0.8%     34.8 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>>>::hash_u64::<&str>` |
|   0.8%     34.0 | `recached_server::clients::execute_and_record_ref` |
|   0.7%     28.4 | `_rjem_sdallocx` |
|   0.6%     27.2 | `<std::hash::random::RandomState as core::hash::BuildHasher>::hash_one::<&str>` |
|   0.6%     25.4 | `<std::sys::pal::unix::time::Timespec>::now` |
|   0.6%     24.8 | `<core_engine::resp::Value>::serialize_into` |
|   0.6%     24.6 | `<recached_server::replication::ReplHub>::stripe_for` |
|   0.6%     24.2 | `recached_server::run::{closure#0}::{closure#27}::{closure#0}` |
|   0.5%     23.3 | `recached_server::propagation::broadcast_for_at` |
|   0.5%     21.6 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|   0.5%     20.5 | `recached_server::propagation::apply_eviction_effects::{closure#0}` |
|   0.5%     19.4 | `<core_engine::request::CommandRef>::to_command` |
|   0.4%     19.0 | `libc.so.6:+1c79c7` |
|   0.4%     18.2 | `libc.so.6:+1c7d91` |
|   0.4%     16.7 | `core::ptr::drop_glue::<<recached_server::replication::ReplHub>::lock_stripes<core::iter::sources::empty::Empt…` |

### Inclusive

| share | cycles/op | function |
|---:|---:|---|
| 100.0%   4249.3 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|  99.0%   4204.8 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|  98.6%   4189.2 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|  98.1%   4167.5 | `recached_server::run::{closure#0}::{closure#27}::{closure#0}` |
|  97.4%   4140.5 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|  74.3%   3155.2 | `recached_server::propagation::execute_ordered_write::{closure#0}` |
|  61.6%   2616.3 | `recached_server::propagation::execute_locked_write_identified::{closure#0}` |
|  40.8%   1733.2 | `recached_server::clients::execute_and_record_ref` |
|  32.4%   1376.0 | `<core_engine::store::KeyValueStore>::execute_reporting_ref` |
|  29.5%   1252.9 | `<core_engine::store::KeyValueStore>::run_probed_write_ordered::<&core_engine::cmd::Command, <core_engine::sto…` |
|  27.8%   1180.2 | `<core_engine::store::KeyValueStore>::execute_inner_ref` |
|  27.4%   1164.2 | `<core_engine::store::KeyValueStore>::set::<&str, &[u8]>` |
|  22.1%    938.1 | `core_engine::store::write_slot::<&str>` |
|  20.7%    879.7 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|  18.3%    778.0 | `recached_server::propagation::apply_write_effects_identified::{closure#0}` |
|   7.5%    317.0 | `<core_engine::request::RequestDecoder>::decode_any` |
|   7.4%    314.0 | `<std::sys::pal::unix::time::Timespec>::now` |
|   7.4%    312.7 | `recached_server::propagation::broadcast_for_at` |
|   7.3%    308.2 | `libc.so.6:+1c7919` |
|   7.2%    306.7 | `<recached_server::server_state::ServerState>::on_write_logged::{closure#0}` |
|   6.8%    289.4 | `recached_server::propagation::resp_push` |
|   6.4%    273.4 | `<recached_server::replication::ReplHub>::lock_stripes::<core::iter::sources::empty::Empty<&str>>::{closure#0}` |
|   6.3%    266.1 | `libc.so.6:+139b5b` |
|   6.1%    259.3 | `recached_server::clients::record_metrics` |
|   3.8%    162.9 | `[vdso]:+a8c` |
|   3.5%    147.9 | `core_engine::resp::write_header` |
|   2.9%    125.2 | `recached_server::propagation::try_execute_borrowed` |
|   2.9%    124.4 | `<tokio::io::util::flush::Flush<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedW…` |
|   2.7%    114.6 | `core_engine::request::build_ref` |
|   2.6%    110.5 | `<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteH…` |
|   2.5%    107.7 | `core::ptr::drop_glue::<smallvec::SmallVec<[tokio::sync::mutex::MutexGuard<()>; 2]>>` |
|   2.4%    102.8 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   2.4%    100.9 | `<tokio::sync::mutex::Mutex<std::collections::hash::map::HashMap<alloc::string::String, alloc::vec::Vec<recach…` |
|   2.1%     89.3 | `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` |
|   2.1%     87.6 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   1.7%     71.8 | `<core_engine::request::Args>::key_ref` |
|   1.6%     69.9 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>>>::hash_u64::<&str>` |
|   1.5%     63.7 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio_rustls::server::TlsStream<tok…` |
|   1.5%     62.6 | `<tokio::sync::batch_semaphore::Semaphore>::add_permits_locked` |
|   1.5%     62.2 | `core::str::converts::from_utf8` |
|   1.4%     59.6 | `<recached_server::replication::ReplHub>::stripe_for` |
|   1.2%     52.6 | `<tokio::sync::batch_semaphore::Acquire as core::future::future::Future>::poll` |
|   1.1%     48.1 | `core_engine::store::entry_size` |
|   1.1%     47.9 | `_rjem_malloc` |
|   1.1%     47.4 | `<core_engine::store::SlotRef as core::ops::drop::Drop>::drop` |
