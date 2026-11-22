samples: 18382, user cycles/op 5188

### Stack contains (inclusive)

| share | cycles/op | label |
|---:|---:|---|
|   9.1%    469.6 | `decode (all)` |
|   0.0%      0.0 | `decode: general Value path (Args::general)` |
|   0.0%      0.0 | `decode: from_value` |
|   0.0%      0.0 | `decode: build (owned MSET/DEL)` |
|   0.0%      0.0 | `to_command (borrowed -> owned)` |
|  35.1%   1820.5 | `borrowed execute (try_execute_borrowed)` |
|   0.0%      0.0 | `ordered write entry (execute_ordered_write)` |
|  26.0%   1348.7 | `store execute (all)` |
|   0.0%      0.0 | `store: Command clone` |
|   2.4%    121.9 | `store: key hash (sip)` |
|   0.0%      0.0 | `store: entry_state` |
|   0.0%      0.0 | `store: extract_key` |
|   4.9%    256.5 | `clock reads` |
|   6.3%    325.1 | `metrics record` |
|   0.0%      0.0 | `reply serialize (Value)` |
|   0.0%      0.0 | `Value drop` |
|   0.0%      0.0 | `Command drop` |
|   0.0%      0.6 | `allocator (all)` |
|   7.5%    391.1 | `select overflow arm (mpsc recv)` |
|  12.4%    644.8 | `BufWriter flush (all)` |
|  13.1%    678.1 | `AofGatedWriter (all)` |
|   2.8%    145.9 | `DeferredAof pop` |
|   4.6%    236.5 | `socket read (poll_read_priv)` |
|   3.9%    202.3 | `socket write (poll_write_priv)` |
|  13.1%    678.1 | `split lock (ReadHalf/WriteHalf)` |
|   4.5%    231.7 | `tokio park/driver` |
|  93.4%   4846.4 | `tokio run_task (scheduler)` |
|   0.0%      0.0 | `task-local scope (with_deferred_aof)` |

### Breakdown: innermost context x leaf class (each sample once)

| context | leaf class | share | cycles/op |
|---|---|---:|---:|
| connection (decode, reply, socket) | code |  26.9% |    1396 |
| connection (decode, reply, socket) | libc (memcpy, syscall wrappers) |   8.1% |     419 |
| connection (decode, reply, socket) | tokio mpsc |   4.3% |     222 |
| connection (decode, reply, socket) | tokio wake/schedule |   2.0% |     103 |
| **connection (decode, reply, socket)** | **all** | ** 41.2%** | **   2139** |
| store execute | code |  17.8% |     921 |
| store execute | libc (memcpy, syscall wrappers) |   8.2% |     428 |
| **store execute** | **all** | ** 26.0%** | **   1349** |
| tokio scheduler | tokio wake/schedule |  13.1% |     681 |
| tokio scheduler | code |   3.4% |     175 |
| tokio scheduler | tokio io/split |   2.1% |     109 |
| tokio scheduler | libc (memcpy, syscall wrappers) |   0.3% |      16 |
| tokio scheduler | allocator |   0.0% |       1 |
| **tokio scheduler** | **all** | ** 18.9%** | **    982** |
| AOF append/lead/gate | tokio io/split |   6.2% |     323 |
| AOF append/lead/gate | code |   5.0% |     260 |
| AOF append/lead/gate | libc (memcpy, syscall wrappers) |   1.8% |      94 |
| **AOF append/lead/gate** | **all** | ** 13.1%** | **    678** |
| tokio io driver | code |   0.7% |      36 |
| tokio io driver | libc (memcpy, syscall wrappers) |   0.1% |       4 |
| **tokio io driver** | **all** | **  0.8%** | **     40** |

### Self (leaf)

| share | cycles/op | function |
|---:|---:|---|
|  12.6%    653.6 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|  11.4%    590.7 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|   5.8%    300.8 | `libc.so.6:+1c7919` |
|   5.5%    285.0 | `<core_engine::request::RequestDecoder>::decode_any` |
|   3.7%    190.8 | `recached_server::clients::record_metrics` |
|   3.6%    185.4 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|   2.9%    151.5 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|   2.8%    145.9 | `<recached_server::persistence::DeferredAof>::pop` |
|   2.8%    143.6 | `libc.so.6:+1c7d91` |
|   2.7%    141.4 | `recached_server::run::{closure#0}::{closure#27}::{closure#0}` |
|   2.6%    136.0 | `<tokio::sync::mpsc::list::Rx<()>>::pop` |
|   2.4%    123.3 | `<core_engine::store::KeyValueStore>::execute_ref_into` |
|   2.1%    109.5 | `<tokio::runtime::io::registration::Registration>::poll_ready` |
|   2.0%    104.4 | `[vdso]:+a8c` |
|   2.0%    103.0 | `<tokio::sync::task::atomic_waker::AtomicWaker>::register_by_ref` |
|   1.9%    101.0 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   1.7%     89.2 | `<tokio::io::util::flush::Flush<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedW…` |
|   1.7%     85.8 | `<tokio::sync::mpsc::bounded::Receiver<()>>::recv::{closure#0}` |
|   1.6%     83.3 | `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` |
|   1.6%     81.6 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|   1.5%     77.0 | `<tokio::net::tcp::stream::TcpStream>::poll_write_priv` |
|   1.4%     72.2 | `<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteH…` |
|   1.3%     69.7 | `core::str::converts::from_utf8` |
|   1.2%     63.8 | `<tokio::runtime::io::scheduled_io::ScheduledIo>::wake` |
|   1.2%     61.0 | `core_engine::request::build_ref` |
|   1.1%     56.7 | `tokio::runtime::task::waker::clone_waker` |
|   1.1%     55.3 | `<tokio::net::tcp::stream::TcpStream>::poll_read_priv` |
|   1.1%     55.0 | `tokio::runtime::task::waker::wake_by_val` |
|   1.0%     51.1 | `[unknown]` |
|   0.9%     46.0 | `libc.so.6:+175818` |
|   0.7%     38.7 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>>>::hash_u64::<&str>` |
|   0.7%     34.4 | `<tokio::runtime::io::driver::Driver>::turn` |
|   0.7%     34.1 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   0.6%     33.3 | `tokio::runtime::task::waker::drop_waker` |
|   0.6%     30.8 | `libc.so.6:+1c79c7` |
|   0.5%     28.2 | `<std::sys::pal::unix::time::Timespec>::sub_timespec` |
|   0.5%     26.2 | `<tokio::io::util::write_all::WriteAll<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::Ao…` |
|   0.5%     23.7 | `<std::hash::random::RandomState as core::hash::BuildHasher>::hash_one::<&str>` |
|   0.4%     21.7 | `recached_server::propagation::try_execute_borrowed` |
|   0.4%     19.2 | `<tokio::runtime::scheduler::multi_thread::handle::Handle>::schedule_task::{closure#0}` |
|   0.4%     18.3 | `libc.so.6:+1c7e47` |
|   0.3%     16.9 | `<std::sys::pal::unix::time::Timespec>::now` |
|   0.3%     16.4 | `core_engine::resp::write_header` |
|   0.3%     15.8 | `libc.so.6:+1c7dcd` |
|   0.3%     15.5 | `<recached_server::persistence::AOF_DEFERRED::__KEY::{K#0}::{closure#0} as core::ops::function::FnOnce<(core::…` |
|   0.3%     15.5 | `<core_engine::request::Args>::key_ref` |
|   0.3%     15.2 | `[vdso]:+a40` |
|   0.3%     15.2 | `libc.so.6:+139b52` |
|   0.3%     14.4 | `libc.so.6:+1c7da5` |
|   0.3%     14.1 | `libc.so.6:+17581c` |
|   0.3%     13.0 | `libc.so.6:+f6332` |
|   0.2%     12.4 | `[vdso]:+a76` |
|   0.2%     12.1 | `libc.so.6:+139b4b` |
|   0.2%     11.9 | `core_engine::resp::format_i64` |
|   0.2%     11.6 | `<tokio::runtime::scheduler::multi_thread::handle::Handle>::schedule_task` |
|   0.2%     10.7 | `libc.so.6:+1c791d` |
|   0.2%      9.6 | `[vdso]:+ac2` |
|   0.2%      9.3 | `libc.so.6:+175826` |
|   0.2%      9.0 | `libc.so.6:+1754b8` |
|   0.2%      8.7 | `[vdso]:+a8e` |

### Inclusive

| share | cycles/op | function |
|---:|---:|---|
| 100.0%   5187.6 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|  93.4%   4846.4 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|  89.8%   4659.6 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|  86.9%   4507.5 | `recached_server::run::{closure#0}::{closure#27}::{closure#0}` |
|  83.8%   4349.4 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|  35.1%   1820.5 | `recached_server::propagation::try_execute_borrowed` |
|  26.0%   1348.7 | `<core_engine::store::KeyValueStore>::execute_ref_into` |
|  21.3%   1103.7 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|  12.4%    644.8 | `<tokio::io::util::flush::Flush<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedW…` |
|   9.1%    469.6 | `<core_engine::request::RequestDecoder>::decode_any` |
|   9.0%    468.8 | `<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteH…` |
|   7.6%    396.5 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   7.5%    391.1 | `<tokio::sync::mpsc::bounded::Receiver<()>>::recv::{closure#0}` |
|   6.3%    325.1 | `recached_server::clients::record_metrics` |
|   5.8%    300.8 | `libc.so.6:+1c7919` |
|   4.9%    256.5 | `<std::sys::pal::unix::time::Timespec>::now` |
|   4.6%    236.5 | `<tokio::net::tcp::stream::TcpStream>::poll_read_priv` |
|   4.5%    231.7 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::park_internal` |
|   4.2%    216.5 | `<tokio::runtime::time::Driver>::park_internal` |
|   3.9%    202.3 | `<tokio::net::tcp::stream::TcpStream>::poll_write_priv` |
|   3.8%    199.0 | `libc.so.6:+139b5b` |
|   3.8%    197.8 | `<tokio::runtime::io::driver::Driver>::turn` |
|   3.5%    180.0 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   3.3%    169.3 | `<tokio::sync::task::atomic_waker::AtomicWaker>::register_by_ref` |
|   3.0%    157.5 | `<tokio::runtime::io::scheduled_io::ScheduledIo>::wake` |
|   2.8%    146.2 | `core_engine::request::build_ref` |
|   2.8%    145.9 | `<recached_server::persistence::DeferredAof>::pop` |
|   2.8%    143.6 | `libc.so.6:+1c7d91` |
|   2.6%    136.0 | `<tokio::sync::mpsc::list::Rx<()>>::pop` |
|   2.6%    133.2 | `<tokio::runtime::io::registration::Registration>::poll_ready` |
|   2.0%    104.4 | `[vdso]:+a8c` |
|   1.9%     98.5 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>>>::hash_u64::<&str>` |
|   1.8%     93.7 | `tokio::runtime::task::waker::wake_by_val` |
|   1.6%     85.2 | `<core_engine::request::Args>::key_ref` |
|   1.6%     83.3 | `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` |
|   1.6%     81.6 | `libc.so.6:+17319c` |
|   1.6%     81.6 | `std::sys::backtrace::__rust_begin_short_backtrace::<<tokio::runtime::blocking::pool::Spawner>::spawn_thread::…` |
|   1.6%     81.6 | `<std::thread::lifecycle::spawn_unchecked<<tokio::runtime::blocking::pool::Spawner>::spawn_thread::{closure#0}…` |
|   1.6%     81.6 | `libc.so.6:+edc42` |
|   1.6%     81.6 | `<std::sys::thread::unix::Thread>::new::thread_start` |
|   1.3%     69.7 | `core::str::converts::from_utf8` |
|   1.1%     56.7 | `tokio::runtime::task::waker::clone_waker` |
|   1.0%     51.1 | `[unknown]` |
|   1.0%     50.2 | `core_engine::resp::write_header` |
|   0.9%     47.1 | `<std::hash::random::RandomState as core::hash::BuildHasher>::hash_one::<&str>` |
|   0.9%     46.0 | `libc.so.6:+175818` |
|   0.7%     37.3 | `libc.so.6:+1754b3` |
|   0.6%     33.3 | `tokio::runtime::task::waker::drop_waker` |
|   0.6%     33.3 | `<tokio::io::util::write_all::WriteAll<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::Ao…` |
|   0.6%     32.2 | `libc.so.6:+175813` |
|   0.6%     30.8 | `<tokio::runtime::scheduler::multi_thread::handle::Handle>::schedule_task` |
|   0.6%     30.8 | `libc.so.6:+1c79c7` |
|   0.5%     28.2 | `<std::sys::pal::unix::time::Timespec>::sub_timespec` |
|   0.4%     23.1 | `<tokio::runtime::scheduler::multi_thread::stats::Stats>::end_processing_scheduled_tasks` |
|   0.4%     19.2 | `<tokio::runtime::scheduler::multi_thread::handle::Handle>::schedule_task::{closure#0}` |
|   0.4%     18.3 | `libc.so.6:+1c7e47` |
|   0.3%     15.8 | `libc.so.6:+1c7dcd` |
|   0.3%     15.5 | `<recached_server::persistence::AOF_DEFERRED::__KEY::{K#0}::{closure#0} as core::ops::function::FnOnce<(core::…` |
|   0.3%     15.2 | `[vdso]:+a40` |
|   0.3%     15.2 | `libc.so.6:+139b52` |
