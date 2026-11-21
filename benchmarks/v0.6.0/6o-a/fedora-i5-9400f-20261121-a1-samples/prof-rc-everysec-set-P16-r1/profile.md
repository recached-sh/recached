samples: 28391, user cycles/op 5686

### Stack contains (inclusive)

| share | cycles/op | label |
|---:|---:|---|
|  28.9%   1646.1 | `frame build (broadcast_for_at, all)` |
|  14.1%    800.7 | `frame build: allocator` |
|  12.6%    714.0 | `frame build: fmt` |
|   0.7%     41.9 | `owned command (to_command)` |
|   5.0%    284.4 | `stripe locks` |
|   5.6%    320.4 | `on_write_logged (all)` |
|   0.0%      0.0 | `append_client_write (queue + ticket)` |
|   1.6%     92.5 | `gate poll_settled (all)` |
|   0.4%     25.2 | `AOF lead/perform (all)` |
|   0.0%      0.0 | `AOF write(2) wrapper` |
|   0.0%      0.0 | `yield_now` |
|   5.8%    332.5 | `clock reads` |
|   0.0%      0.0 | `now_unix_ms` |
|   4.5%    256.0 | `metrics record` |
|  30.9%   1754.4 | `store execute (all)` |
|   0.5%     30.8 | `reply serialize` |
|   5.6%    320.2 | `decode` |
|  14.0%    793.7 | `allocator (all)` |
|   0.0%      0.0 | `socket send/recv wrappers` |
|   2.0%    114.0 | `BufWriter flush` |
|   0.0%      0.0 | `Value drop` |
|   0.0%      0.0 | `Command drop` |

### Breakdown: innermost context x leaf class (each sample once)

| context | leaf class | share | cycles/op |
|---|---|---:|---:|
| frame build (broadcast_for_at) | allocator |  14.1% |     801 |
| frame build (broadcast_for_at) | code |  12.7% |     720 |
| frame build (broadcast_for_at) | libc (memcpy, syscall wrappers) |   2.2% |     125 |
| **frame build (broadcast_for_at)** | **all** | ** 28.9%** | **   1646** |
| store execute | code |  17.6% |    1001 |
| store execute | libc (memcpy, syscall wrappers) |   8.2% |     465 |
| store execute | allocator |   0.1% |       6 |
| **store execute** | **all** | ** 25.9%** | **   1472** |
| connection (decode, reply, socket) | code |  15.1% |     857 |
| connection (decode, reply, socket) | libc (memcpy, syscall wrappers) |   3.7% |     211 |
| connection (decode, reply, socket) | allocator |   0.7% |      39 |
| connection (decode, reply, socket) | tokio mpsc |   0.5% |      28 |
| connection (decode, reply, socket) | tokio wake/schedule |   0.1% |       6 |
| **connection (decode, reply, socket)** | **all** | ** 20.1%** | **   1141** |
| ordered write path (rest) | code |   7.1% |     404 |
| ordered write path (rest) | tokio mpsc |   1.3% |      72 |
| ordered write path (rest) | libc (memcpy, syscall wrappers) |   1.0% |      57 |
| ordered write path (rest) | allocator |   0.1% |       7 |
| **ordered write path (rest)** | **all** | **  9.5%** | **    540** |
| on_write_logged (frame copy, dispatch) | code |   4.7% |     266 |
| on_write_logged (frame copy, dispatch) | libc (memcpy, syscall wrappers) |   0.5% |      28 |
| on_write_logged (frame copy, dispatch) | allocator |   0.5% |      26 |
| **on_write_logged (frame copy, dispatch)** | **all** | **  5.6%** | **    320** |
| stripe locks | code |   4.0% |     225 |
| stripe locks | tokio mpsc |   1.0% |      59 |
| **stripe locks** | **all** | **  5.0%** | **    284** |
| AOF append/lead/gate | tokio io/split |   1.2% |      66 |
| AOF append/lead/gate | code |   0.7% |      37 |
| AOF append/lead/gate | libc (memcpy, syscall wrappers) |   0.4% |      22 |
| AOF append/lead/gate | tokio wake/schedule |   0.2% |      14 |
| AOF append/lead/gate | allocator |   0.2% |      12 |
| AOF append/lead/gate | atomics/Arc drop |   0.0% |       0 |
| **AOF append/lead/gate** | **all** | **  2.7%** | **    151** |
| tokio scheduler | tokio wake/schedule |   1.4% |      77 |
| tokio scheduler | code |   0.7% |      40 |
| tokio scheduler | tokio io/split |   0.2% |       9 |
| tokio scheduler | libc (memcpy, syscall wrappers) |   0.0% |       2 |
| tokio scheduler | allocator |   0.0% |       0 |
| **tokio scheduler** | **all** | **  2.2%** | **    128** |
| tokio io driver | code |   0.0% |       2 |
| tokio io driver | libc (memcpy, syscall wrappers) |   0.0% |       1 |
| **tokio io driver** | **all** | **  0.1%** | **      3** |

### Self (leaf)

| share | cycles/op | function |
|---:|---:|---|
|   8.2%    467.6 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   5.3%    303.8 | `libc.so.6:+1c7919` |
|   4.6%    261.6 | `<recached_server::server_state::ServerState>::on_write_logged::{closure#0}` |
|   3.8%    216.3 | `_rjem_je_arena_ralloc` |
|   3.5%    196.7 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|   3.4%    193.3 | `alloc::fmt::format::format_inner` |
|   3.3%    190.5 | `do_rallocx` |
|   3.1%    174.2 | `<core_engine::request::RequestDecoder>::decode_any` |
|   3.0%    169.4 | `[vdso]:+a8c` |
|   2.5%    141.6 | `_rjem_je_arena_ralloc_no_move` |
|   2.3%    129.0 | `recached_server::propagation::execute_ordered_write::{closure#0}` |
|   2.2%    126.0 | `recached_server::clients::record_metrics` |
|   2.1%    117.2 | `core::fmt::write` |
|   1.8%    103.9 | `<alloc::string::String as core::fmt::Write>::write_str` |
|   1.6%     92.3 | `recached_server::propagation::resp_push` |
|   1.6%     89.7 | `_rjem_malloc` |
|   1.5%     88.1 | `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` |
|   1.5%     85.1 | `<core_engine::store::KeyValueStore>::set::<&str, &[u8]>` |
|   1.5%     83.3 | `<recached_server::replication::ReplHub>::lock_stripes::<core::iter::sources::empty::Empty<&str>>::{closure#0}` |
|   1.4%     78.9 | `<core_engine::store::KeyValueStore>::execute_reporting_ref` |
|   1.3%     71.9 | `<tokio::sync::batch_semaphore::Semaphore>::add_permits_locked` |
|   1.2%     67.1 | `recached_server::propagation::apply_write_effects_identified::{closure#0}` |
|   1.2%     67.1 | `core::str::converts::from_utf8` |
|   1.1%     64.9 | `recached_server::propagation::try_execute_borrowed` |
|   1.1%     64.5 | `_rjem_sdallocx` |
|   1.1%     59.9 | `<core::fmt::Formatter>::pad_integral` |
|   1.0%     58.7 | `<usize>::_fmt_inner` |
|   1.0%     56.5 | `recached_server::propagation::execute_locked_write_identified::{closure#0}` |
|   0.9%     52.3 | `<tokio::sync::batch_semaphore::Acquire as core::future::future::Future>::poll` |
|   0.9%     51.5 | `<tokio::sync::mutex::Mutex<std::collections::hash::map::HashMap<alloc::string::String, alloc::vec::Vec<recach…` |
|   0.9%     50.1 | `core_engine::store::entry_size` |
|   0.9%     49.7 | `<alloc::raw_vec::RawVecInner<_>>::reserve::do_reserve_and_handle::<alloc::alloc::Global>` |
|   0.9%     48.7 | `core_engine::request::build_ref` |
|   0.9%     48.5 | `core_engine::store::lock_stripe` |
|   0.8%     48.1 | `<alloc::raw_vec::RawVecInner>::finish_grow` |
|   0.8%     47.7 | `<core_engine::store::KeyValueStore>::run_probed_write_ordered::<&core_engine::cmd::Command, <core_engine::sto…` |
|   0.7%     40.3 | `core::ptr::drop_glue::<smallvec::SmallVec<[tokio::sync::mutex::MutexGuard<()>; 2]>>` |
|   0.7%     38.5 | `<usize as core::fmt::Display>::fmt` |
|   0.7%     37.9 | `recached_server::clients::execute_and_record_ref` |
|   0.7%     37.9 | `<core_engine::store::SlotRef as core::ops::drop::Drop>::drop` |
|   0.6%     35.4 | `<std::sys::pal::unix::time::Timespec>::sub_timespec` |
|   0.6%     33.4 | `_rjem_realloc` |
|   0.5%     30.8 | `<core::fmt::Formatter>::pad_integral::write_prefix` |
|   0.5%     30.2 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>>>::hash_u64::<&str>` |
|   0.5%     29.2 | `recached_server::run::{closure#0}::{closure#27}::{closure#0}` |

### Inclusive

| share | cycles/op | function |
|---:|---:|---|
| 100.0%   5686.0 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|  99.2%   5641.8 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|  98.9%   5622.0 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|  98.6%   5606.1 | `recached_server::run::{closure#0}::{closure#27}::{closure#0}` |
|  98.0%   5573.1 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|  79.9%   4541.3 | `recached_server::propagation::execute_ordered_write::{closure#0}` |
|  70.3%   3996.3 | `recached_server::propagation::execute_locked_write_identified::{closure#0}` |
|  37.5%   2130.7 | `recached_server::propagation::apply_write_effects_identified::{closure#0}` |
|  30.9%   1754.4 | `recached_server::clients::execute_and_record_ref` |
|  28.9%   1646.1 | `recached_server::propagation::broadcast_for_at` |
|  28.6%   1624.8 | `recached_server::propagation::resp_push` |
|  24.1%   1371.5 | `<core_engine::store::KeyValueStore>::execute_reporting_ref` |
|  21.4%   1215.1 | `<core_engine::store::KeyValueStore>::run_probed_write_ordered::<&core_engine::cmd::Command, <core_engine::sto…` |
|  20.1%   1141.6 | `<core_engine::store::KeyValueStore>::execute_inner_ref` |
|  19.9%   1129.8 | `<core_engine::store::KeyValueStore>::set::<&str, &[u8]>` |
|  16.1%    914.3 | `core_engine::store::write_slot::<&str>` |
|  15.0%    855.2 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|  13.7%    780.1 | `<alloc::raw_vec::RawVecInner<_>>::reserve::do_reserve_and_handle::<alloc::alloc::Global>` |
|  12.8%    730.4 | `<alloc::raw_vec::RawVecInner>::finish_grow` |
|  12.6%    714.0 | `alloc::fmt::format::format_inner` |
|  11.4%    648.5 | `do_rallocx` |
|   8.0%    452.6 | `core::fmt::write` |
|   7.8%    444.8 | `_rjem_je_arena_ralloc` |
|   5.8%    332.5 | `<std::sys::pal::unix::time::Timespec>::now` |
|   5.6%    320.2 | `<core_engine::request::RequestDecoder>::decode_any` |
|   5.6%    318.2 | `<recached_server::server_state::ServerState>::on_write_logged::{closure#0}` |
|   5.3%    303.8 | `libc.so.6:+1c7919` |
|   4.9%    278.8 | `libc.so.6:+139b5b` |
|   4.8%    274.2 | `<recached_server::replication::ReplHub>::lock_stripes::<core::iter::sources::empty::Empty<&str>>::{closure#0}` |
|   4.5%    256.0 | `recached_server::clients::record_metrics` |
|   4.2%    236.7 | `<usize as core::fmt::Display>::fmt` |
|   3.0%    169.4 | `[vdso]:+a8c` |
|   2.6%    148.2 | `recached_server::propagation::try_execute_borrowed` |
|   2.6%    147.6 | `<alloc::string::String as core::fmt::Write>::write_str` |
|   2.5%    141.6 | `_rjem_je_arena_ralloc_no_move` |
|   2.3%    129.0 | `<tokio::io::util::flush::Flush<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedW…` |
|   2.2%    124.0 | `core_engine::request::build_ref` |
|   2.0%    114.0 | `<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteH…` |
|   2.0%    112.2 | `core::ptr::drop_glue::<smallvec::SmallVec<[tokio::sync::mutex::MutexGuard<()>; 2]>>` |
|   1.9%    110.8 | `<tokio::sync::mutex::Mutex<std::collections::hash::map::HashMap<alloc::string::String, alloc::vec::Vec<recach…` |
|   1.9%    107.7 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   1.6%     92.5 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   1.6%     90.7 | `<core::fmt::Formatter>::pad_integral` |
|   1.6%     89.7 | `_rjem_malloc` |
|   1.5%     88.1 | `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` |
