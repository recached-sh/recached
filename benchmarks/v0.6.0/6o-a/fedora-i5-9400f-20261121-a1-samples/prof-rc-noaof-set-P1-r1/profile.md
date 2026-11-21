samples: 20148, user cycles/op 5977

### Stack contains (inclusive)

| share | cycles/op | label |
|---:|---:|---|
|   0.0%      0.0 | `frame build (broadcast_for_at, all)` |
|   0.0%      0.0 | `frame build: allocator` |
|   0.0%      0.0 | `frame build: fmt` |
|   0.0%      0.0 | `owned command (to_command)` |
|   0.0%      0.0 | `stripe locks` |
|   0.0%      0.0 | `on_write_logged (all)` |
|   0.0%      0.0 | `append_client_write (queue + ticket)` |
|   3.4%    205.3 | `gate poll_settled (all)` |
|   0.0%      0.0 | `AOF lead/perform (all)` |
|   0.0%      0.0 | `AOF write(2) wrapper` |
|   0.0%      0.0 | `yield_now` |
|   3.9%    234.6 | `clock reads` |
|   0.0%      0.0 | `now_unix_ms` |
|   5.1%    304.1 | `metrics record` |
|  31.6%   1890.2 | `store execute (all)` |
|   0.8%     50.7 | `reply serialize` |
|   8.7%    519.7 | `decode` |
|   0.8%     45.1 | `allocator (all)` |
|   0.0%      0.0 | `socket send/recv wrappers` |
|   9.1%    542.3 | `BufWriter flush` |
|   0.0%      0.0 | `Value drop` |
|   0.0%      0.0 | `Command drop` |

### Breakdown: innermost context x leaf class (each sample once)

| context | leaf class | share | cycles/op |
|---|---|---:|---:|
| connection (decode, reply, socket) | code |  25.9% |    1549 |
| connection (decode, reply, socket) | libc (memcpy, syscall wrappers) |   6.5% |     386 |
| connection (decode, reply, socket) | tokio mpsc |   3.6% |     213 |
| connection (decode, reply, socket) | tokio wake/schedule |   1.7% |     104 |
| **connection (decode, reply, socket)** | **all** | ** 37.7%** | **   2252** |
| store execute | code |  22.9% |    1370 |
| store execute | libc (memcpy, syscall wrappers) |   8.0% |     476 |
| store execute | allocator |   0.7% |      44 |
| **store execute** | **all** | ** 31.6%** | **   1890** |
| tokio scheduler | tokio wake/schedule |  12.3% |     734 |
| tokio scheduler | code |   2.7% |     164 |
| tokio scheduler | tokio io/split |   2.0% |     117 |
| tokio scheduler | libc (memcpy, syscall wrappers) |   0.2% |      14 |
| tokio scheduler | allocator |   0.0% |       1 |
| **tokio scheduler** | **all** | ** 17.2%** | **   1030** |
| AOF append/lead/gate | tokio io/split |   5.6% |     335 |
| AOF append/lead/gate | code |   4.6% |     278 |
| AOF append/lead/gate | libc (memcpy, syscall wrappers) |   2.6% |     153 |
| **AOF append/lead/gate** | **all** | ** 12.8%** | **    766** |
| tokio io driver | code |   0.6% |      34 |
| tokio io driver | libc (memcpy, syscall wrappers) |   0.1% |       5 |
| **tokio io driver** | **all** | **  0.7%** | **     39** |

### Self (leaf)

| share | cycles/op | function |
|---:|---:|---|
|  10.6%    634.8 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   9.7%    581.7 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|   5.2%    309.1 | `libc.so.6:+1c7919` |
|   4.3%    255.1 | `<core_engine::request::RequestDecoder>::decode_any` |
|   3.0%    179.5 | `recached_server::clients::record_metrics` |
|   2.8%    170.3 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|   2.8%    167.0 | `<recached_server::persistence::DeferredAof>::pop` |
|   2.8%    165.2 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|   2.5%    149.8 | `<tokio::sync::mpsc::list::Rx<()>>::pop` |
|   2.4%    142.4 | `recached_server::propagation::try_execute_borrowed` |
|   2.2%    132.3 | `recached_server::run::{closure#0}::{closure#27}::{closure#0}` |
|   2.1%    127.3 | `libc.so.6:+1c7d91` |
|   2.0%    117.5 | `<tokio::runtime::io::registration::Registration>::poll_ready` |
|   1.9%    115.7 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|   1.9%    114.8 | `<core_engine::store::KeyValueStore>::execute_ref_into` |
|   1.9%    113.0 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   1.9%    112.7 | `core::str::converts::from_utf8` |
|   1.8%    105.3 | `[vdso]:+a8c` |
|   1.7%    104.1 | `<tokio::sync::task::atomic_waker::AtomicWaker>::register_by_ref` |
|   1.7%    102.0 | `<core_engine::store::KeyValueStore>::set::<&str, &[u8]>` |
|   1.4%     85.7 | `<tokio::io::util::flush::Flush<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedW…` |
|   1.3%     79.2 | `core_engine::request::build_ref` |
|   1.3%     77.7 | `libc.so.6:+175818` |
|   1.3%     75.3 | `<tokio::runtime::io::scheduled_io::ScheduledIo>::wake` |
|   1.2%     73.3 | `tokio::runtime::task::waker::clone_waker` |
|   1.2%     72.7 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>>>::hash_u64::<&str>` |
|   1.2%     72.1 | `<tokio::net::tcp::stream::TcpStream>::poll_write_priv` |
|   1.1%     66.7 | `core_engine::store::entry_size` |
|   1.1%     63.5 | `<tokio::sync::mpsc::bounded::Receiver<()>>::recv::{closure#0}` |
|   1.1%     62.9 | `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` |
|   1.0%     60.5 | `<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteH…` |
|   1.0%     57.8 | `[unknown]` |
|   0.9%     56.1 | `<tokio::net::tcp::stream::TcpStream>::poll_read_priv` |
|   0.9%     53.7 | `core_engine::store::lock_stripe` |
|   0.9%     53.4 | `tokio::runtime::task::waker::wake_by_val` |
|   0.9%     53.4 | `<core_engine::store::SlotRef as core::ops::drop::Drop>::drop` |
|   0.9%     52.2 | `<core_engine::store::KeyIndex>::sync` |
|   0.9%     51.6 | `core_engine::store::write_slot::<&str>` |
|   0.8%     48.9 | `libc.so.6:+1c79c7` |
|   0.8%     47.2 | `<core_engine::resp::Value>::serialize_into` |
|   0.7%     40.3 | `core::ptr::drop_glue::<core_engine::resp::Value>` |
|   0.6%     38.3 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   0.6%     37.1 | `<tokio::io::util::write_all::WriteAll<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::Ao…` |
|   0.6%     35.0 | `tokio::runtime::task::waker::drop_waker` |
|   0.6%     34.1 | `<tokio::runtime::io::driver::Driver>::turn` |

### Inclusive

| share | cycles/op | function |
|---:|---:|---|
| 100.0%   5976.9 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|  93.8%   5603.4 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|  90.9%   5431.9 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|  88.1%   5266.4 | `recached_server::run::{closure#0}::{closure#27}::{closure#0}` |
|  85.7%   5121.3 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|  40.8%   2439.9 | `recached_server::propagation::try_execute_borrowed` |
|  31.6%   1890.2 | `<core_engine::store::KeyValueStore>::execute_ref_into` |
|  24.7%   1478.2 | `<core_engine::store::KeyValueStore>::set::<&str, &[u8]>` |
|  20.1%   1199.3 | `core_engine::store::write_slot::<&str>` |
|  18.0%   1074.8 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|  12.0%    716.1 | `<tokio::io::util::flush::Flush<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedW…` |
|   9.1%    542.3 | `<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteH…` |
|   8.7%    519.7 | `<core_engine::request::RequestDecoder>::decode_any` |
|   8.1%    481.8 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   6.6%    393.4 | `<tokio::sync::mpsc::bounded::Receiver<()>>::recv::{closure#0}` |
|   5.2%    309.1 | `libc.so.6:+1c7919` |
|   5.1%    304.1 | `recached_server::clients::record_metrics` |
|   4.2%    252.2 | `<tokio::net::tcp::stream::TcpStream>::poll_read_priv` |
|   4.2%    251.6 | `<tokio::net::tcp::stream::TcpStream>::poll_write_priv` |
|   3.9%    234.6 | `<std::sys::pal::unix::time::Timespec>::now` |
|   3.9%    232.9 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::park_internal` |
|   3.6%    216.3 | `<tokio::runtime::time::Driver>::park_internal` |
|   3.4%    205.3 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   3.4%    204.7 | `core_engine::request::build_ref` |
|   3.3%    198.8 | `<tokio::runtime::io::driver::Driver>::turn` |
|   3.0%    180.1 | `<tokio::sync::task::atomic_waker::AtomicWaker>::register_by_ref` |
|   2.9%    172.6 | `libc.so.6:+139b5b` |
|   2.8%    167.0 | `<recached_server::persistence::DeferredAof>::pop` |
|   2.7%    159.3 | `<tokio::runtime::io::scheduled_io::ScheduledIo>::wake` |
|   2.5%    149.8 | `<tokio::sync::mpsc::list::Rx<()>>::pop` |
|   2.5%    149.8 | `<tokio::runtime::io::registration::Registration>::poll_ready` |
|   2.3%    136.2 | `<core_engine::store::KeyIndex>::sync` |
|   2.1%    127.3 | `libc.so.6:+1c7d91` |
|   2.1%    125.5 | `<core_engine::request::Args>::key_ref` |
|   1.9%    115.7 | `<std::sys::thread::unix::Thread>::new::thread_start` |
|   1.9%    115.7 | `<std::thread::lifecycle::spawn_unchecked<<tokio::runtime::blocking::pool::Spawner>::spawn_thread::{closure#0}…` |
|   1.9%    115.7 | `std::sys::backtrace::__rust_begin_short_backtrace::<<tokio::runtime::blocking::pool::Spawner>::spawn_thread::…` |
|   1.9%    115.7 | `libc.so.6:+edc42` |
|   1.9%    115.7 | `libc.so.6:+17319c` |
|   1.9%    115.1 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>>>::hash_u64::<&str>` |
|   1.9%    112.7 | `core::str::converts::from_utf8` |
|   1.8%    105.3 | `[vdso]:+a8c` |
|   1.4%     84.0 | `tokio::runtime::task::waker::wake_by_val` |
|   1.3%     77.7 | `libc.so.6:+175818` |
|   1.2%     73.3 | `tokio::runtime::task::waker::clone_waker` |
