samples: 26222, user cycles/op 12175

### Stack contains (inclusive)

| share | cycles/op | label |
|---:|---:|---|
|  17.9%   2184.5 | `frame build (broadcast_for_at, all)` |
|   8.5%   1036.3 | `frame build: allocator` |
|   7.5%    908.1 | `frame build: fmt` |
|   0.7%     86.4 | `owned command (to_command)` |
|   5.6%    680.2 | `stripe locks` |
|   3.9%    472.2 | `on_write_logged (all)` |
|   0.0%      0.0 | `append_client_write (queue + ticket)` |
|   9.2%   1118.0 | `gate poll_settled (all)` |
|   1.6%    192.2 | `AOF lead/perform (all)` |
|   0.0%      0.0 | `AOF write(2) wrapper` |
|   0.0%      0.0 | `yield_now` |
|   3.3%    399.3 | `clock reads` |
|   0.0%      0.0 | `now_unix_ms` |
|   2.8%    346.4 | `metrics record` |
|  20.6%   2506.7 | `store execute (all)` |
|   0.2%     20.4 | `reply serialize` |
|   4.2%    507.0 | `decode` |
|   9.0%   1099.4 | `allocator (all)` |
|   0.0%      0.0 | `socket send/recv wrappers` |
|  12.4%   1507.5 | `BufWriter flush` |
|   0.0%      0.0 | `Value drop` |
|   0.0%      0.0 | `Command drop` |

### Breakdown: innermost context x leaf class (each sample once)

| context | leaf class | share | cycles/op |
|---|---|---:|---:|
| connection (decode, reply, socket) | code |  15.8% |    1929 |
| connection (decode, reply, socket) | libc (memcpy, syscall wrappers) |   2.6% |     312 |
| connection (decode, reply, socket) | tokio mpsc |   2.1% |     254 |
| connection (decode, reply, socket) | tokio wake/schedule |   0.9% |     111 |
| connection (decode, reply, socket) | allocator |   0.3% |      33 |
| **connection (decode, reply, socket)** | **all** | ** 21.7%** | **   2639** |
| frame build (broadcast_for_at) | allocator |   8.5% |    1036 |
| frame build (broadcast_for_at) | code |   8.1% |     988 |
| frame build (broadcast_for_at) | libc (memcpy, syscall wrappers) |   1.3% |     161 |
| **frame build (broadcast_for_at)** | **all** | ** 17.9%** | **   2184** |
| store execute | code |  12.7% |    1543 |
| store execute | libc (memcpy, syscall wrappers) |   4.8% |     587 |
| store execute | allocator |   0.1% |       8 |
| **store execute** | **all** | ** 17.6%** | **   2139** |
| tokio scheduler | tokio wake/schedule |   9.2% |    1124 |
| tokio scheduler | code |   3.4% |     418 |
| tokio scheduler | tokio io/split |   0.9% |     113 |
| tokio scheduler | libc (memcpy, syscall wrappers) |   0.2% |      28 |
| tokio scheduler | tokio mpsc |   0.0% |       0 |
| tokio scheduler | allocator |   0.0% |       0 |
| **tokio scheduler** | **all** | ** 13.8%** | **   1684** |
| AOF append/lead/gate | tokio io/split |   5.4% |     663 |
| AOF append/lead/gate | code |   4.2% |     517 |
| AOF append/lead/gate | tokio wake/schedule |   1.7% |     211 |
| AOF append/lead/gate | libc (memcpy, syscall wrappers) |   1.7% |     202 |
| AOF append/lead/gate | allocator |   0.2% |      30 |
| AOF append/lead/gate | atomics/Arc drop |   0.1% |      10 |
| **AOF append/lead/gate** | **all** | ** 13.4%** | **   1632** |
| ordered write path (rest) | code |   4.4% |     538 |
| ordered write path (rest) | tokio mpsc |   0.8% |      95 |
| ordered write path (rest) | libc (memcpy, syscall wrappers) |   0.4% |      54 |
| ordered write path (rest) | allocator |   0.1% |      11 |
| **ordered write path (rest)** | **all** | **  5.7%** | **    698** |
| stripe locks | code |   4.3% |     527 |
| stripe locks | tokio mpsc |   1.3% |     153 |
| **stripe locks** | **all** | **  5.6%** | **    680** |
| on_write_logged (frame copy, dispatch) | code |   2.8% |     344 |
| on_write_logged (frame copy, dispatch) | allocator |   0.8% |     103 |
| on_write_logged (frame copy, dispatch) | libc (memcpy, syscall wrappers) |   0.2% |      26 |
| **on_write_logged (frame copy, dispatch)** | **all** | **  3.9%** | **    472** |
| tokio io driver | code |   0.3% |      37 |
| tokio io driver | libc (memcpy, syscall wrappers) |   0.1% |      10 |
| **tokio io driver** | **all** | **  0.4%** | **     46** |

### Self (leaf)

| share | cycles/op | function |
|---:|---:|---|
|   6.8%    831.1 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|   5.6%    683.0 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   3.2%    384.9 | `libc.so.6:+1c7919` |
|   2.8%    340.8 | `<recached_server::server_state::ServerState>::on_write_logged::{closure#0}` |
|   2.6%    319.0 | `do_rallocx` |
|   2.5%    305.0 | `recached_server::run::{closure#0}::{closure#27}::{closure#0}` |
|   2.1%    260.9 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|   2.1%    258.6 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|   1.9%    228.9 | `_rjem_je_arena_ralloc` |
|   1.8%    225.2 | `<recached_server::replication::ReplHub>::lock_stripes::<core::iter::sources::empty::Empty<&str>>::{closure#0}` |
|   1.8%    216.4 | `alloc::fmt::format::format_inner` |
|   1.7%    211.3 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|   1.7%    203.8 | `<recached_server::persistence::DeferredAof>::pop` |
|   1.6%    197.3 | `<core_engine::request::RequestDecoder>::decode_any` |
|   1.6%    195.0 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio_rustls::server::TlsStream<tok…` |
|   1.5%    187.6 | `recached_server::clients::record_metrics` |
|   1.5%    182.0 | `[vdso]:+a8c` |
|   1.5%    177.8 | `<tokio::sync::mpsc::list::Rx<()>>::pop` |
|   1.4%    166.2 | `recached_server::propagation::execute_ordered_write::{closure#0}` |
|   1.4%    165.3 | `_rjem_je_arena_ralloc_no_move` |
|   1.3%    157.9 | `core::fmt::write` |
|   1.3%    155.5 | `_rjem_malloc` |
|   1.1%    136.5 | `<alloc::string::String as core::fmt::Write>::write_str` |
|   1.1%    135.6 | `recached_server::propagation::resp_push` |
|   1.1%    132.8 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   1.1%    129.1 | `<tokio::sync::notify::NotifiedProject>::poll_notified` |
|   1.0%    125.4 | `<core_engine::store::KeyValueStore>::execute_reporting_ref` |
|   1.0%    119.3 | `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` |
|   1.0%    116.5 | `core::str::converts::from_utf8` |
|   0.9%    114.2 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   0.9%    112.8 | `<tokio::sync::batch_semaphore::Acquire as core::future::future::Future>::poll` |
|   0.9%    112.4 | `<tokio::runtime::io::registration::Registration>::poll_ready` |
|   0.9%    111.0 | `<tokio::sync::task::atomic_waker::AtomicWaker>::register_by_ref` |
|   0.9%    108.2 | `core_engine::store::incr_by::<&str>` |
|   0.9%    105.4 | `core_engine::request::build_ref` |
|   0.8%    103.1 | `<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteH…` |
|   0.8%    100.3 | `tokio::runtime::task::waker::wake_by_val` |
|   0.8%     99.8 | `recached_server::propagation::try_execute_borrowed` |
|   0.8%     95.6 | `<tokio::sync::mutex::Mutex<std::collections::hash::map::HashMap<alloc::string::String, alloc::vec::Vec<recach…` |
|   0.8%     95.2 | `<tokio::sync::batch_semaphore::Semaphore>::add_permits_locked` |
|   0.8%     92.4 | `_rjem_sdallocx` |
|   0.8%     91.9 | `<tokio::io::util::flush::Flush<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedW…` |
|   0.8%     91.5 | `<core::fmt::Formatter>::pad_integral` |
|   0.7%     88.7 | `<usize>::_fmt_inner` |
|   0.7%     84.5 | `tokio::runtime::task::waker::clone_waker` |

### Inclusive

| share | cycles/op | function |
|---:|---:|---|
| 100.0%  12174.5 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|  95.0%  11564.5 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|  92.8%  11302.1 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|  90.7%  11040.8 | `recached_server::run::{closure#0}::{closure#27}::{closure#0}` |
|  88.0%  10712.5 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|  53.7%   6533.5 | `recached_server::propagation::execute_ordered_write::{closure#0}` |
|  45.3%   5513.4 | `recached_server::propagation::execute_locked_write_identified::{closure#0}` |
|  23.4%   2848.9 | `recached_server::propagation::apply_write_effects_identified::{closure#0}` |
|  20.6%   2506.7 | `recached_server::clients::execute_and_record_ref` |
|  17.9%   2184.5 | `recached_server::propagation::broadcast_for_at` |
|  16.9%   2052.2 | `recached_server::propagation::resp_push` |
|  16.5%   2004.3 | `<core_engine::store::KeyValueStore>::execute_reporting_ref` |
|  14.4%   1749.0 | `<core_engine::store::KeyValueStore>::run_probed_write_ordered::<&core_engine::cmd::Command, <core_engine::sto…` |
|  13.8%   1685.8 | `<tokio::io::util::flush::Flush<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedW…` |
|  12.4%   1515.4 | `<core_engine::store::KeyValueStore>::execute_inner_ref` |
|  12.4%   1507.5 | `<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteH…` |
|  12.3%   1495.0 | `core_engine::store::incr_by::<&str>` |
|  11.5%   1404.5 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   9.7%   1185.8 | `core_engine::store::write_slot::<&str>` |
|   9.2%   1124.0 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   9.2%   1118.0 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   8.5%   1028.9 | `<alloc::raw_vec::RawVecInner<_>>::reserve::do_reserve_and_handle::<alloc::alloc::Global>` |
|   8.0%    974.5 | `<alloc::raw_vec::RawVecInner>::finish_grow` |
|   7.5%    908.1 | `alloc::fmt::format::format_inner` |
|   7.0%    850.1 | `do_rallocx` |
|   6.0%    727.1 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio_rustls::server::TlsStream<tok…` |
|   5.2%    636.5 | `<recached_server::replication::ReplHub>::lock_stripes::<core::iter::sources::empty::Empty<&str>>::{closure#0}` |
|   4.9%    598.5 | `core::fmt::write` |
|   4.3%    524.2 | `_rjem_je_arena_ralloc` |
|   4.2%    507.0 | `<core_engine::request::RequestDecoder>::decode_any` |
|   3.9%    469.9 | `<recached_server::server_state::ServerState>::on_write_logged::{closure#0}` |
|   3.6%    439.7 | `<tokio::sync::mpsc::bounded::Receiver<()>>::recv::{closure#0}` |
|   3.3%    399.3 | `<std::sys::pal::unix::time::Timespec>::now` |
|   3.2%    384.9 | `libc.so.6:+1c7919` |
|   2.8%    346.4 | `recached_server::clients::record_metrics` |
|   2.8%    343.1 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::park_internal` |
|   2.6%    315.3 | `libc.so.6:+139b5b` |
|   2.5%    307.8 | `<usize as core::fmt::Display>::fmt` |
|   2.1%    258.6 | `<tokio::net::tcp::stream::TcpStream>::poll_write_priv` |
|   2.1%    252.6 | `core_engine::request::build_ref` |
|   2.0%    248.9 | `<tokio::sync::mutex::Mutex<std::collections::hash::map::HashMap<alloc::string::String, alloc::vec::Vec<recach…` |
|   2.0%    246.5 | `recached_server::persistence::gate_wait::{closure#0}` |
|   2.0%    243.8 | `<tokio::runtime::time::Driver>::park_internal` |
|   1.9%    228.9 | `<recached_server::persistence::AofDone>::wait::{closure#0}` |
|   1.8%    224.7 | `<tokio::net::tcp::stream::TcpStream>::poll_read_priv` |
