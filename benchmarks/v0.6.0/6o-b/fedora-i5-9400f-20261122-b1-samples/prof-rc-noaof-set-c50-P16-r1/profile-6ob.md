samples: 8389, user cycles/op 2245

### Stack contains (inclusive)

| share | cycles/op | label |
|---:|---:|---|
|  14.6%    327.5 | `decode (all)` |
|   0.0%      0.0 | `decode: general Value path (Args::general)` |
|   0.0%      0.0 | `decode: from_value` |
|   0.0%      0.0 | `decode: build (owned MSET/DEL)` |
|   0.0%      0.0 | `to_command (borrowed -> owned)` |
|  74.0%   1661.6 | `borrowed execute (try_execute_borrowed)` |
|   0.0%      0.0 | `ordered write entry (execute_ordered_write)` |
|  58.4%   1311.3 | `store execute (all)` |
|   0.0%      0.0 | `store: Command clone` |
|   3.7%     82.7 | `store: key hash (sip)` |
|   0.0%      0.0 | `store: entry_state` |
|   0.0%      0.0 | `store: extract_key` |
|   8.4%    189.5 | `clock reads` |
|  10.1%    227.5 | `metrics record` |
|   0.8%     17.9 | `reply serialize (Value)` |
|   0.4%      9.9 | `Value drop` |
|   0.0%      0.0 | `Command drop` |
|   0.7%     16.6 | `allocator (all)` |
|   1.0%     22.5 | `select overflow arm (mpsc recv)` |
|   2.0%     45.0 | `BufWriter flush (all)` |
|   3.1%     69.6 | `AofGatedWriter (all)` |
|   0.5%     10.7 | `DeferredAof pop` |
|   0.6%     14.5 | `socket read (poll_read_priv)` |
|   0.5%     10.4 | `socket write (poll_write_priv)` |
|   3.1%     69.6 | `split lock (ReadHalf/WriteHalf)` |
|   0.7%     14.7 | `tokio park/driver` |
|  98.9%   2219.3 | `tokio run_task (scheduler)` |
|   0.0%      0.0 | `task-local scope (with_deferred_aof)` |

### Breakdown: innermost context x leaf class (each sample once)

| context | leaf class | share | cycles/op |
|---|---|---:|---:|
| store execute | code |  39.8% |     892 |
| store execute | libc (memcpy, syscall wrappers) |  17.9% |     402 |
| store execute | allocator |   0.7% |      16 |
| **store execute** | **all** | ** 58.4%** | **   1311** |
| connection (decode, reply, socket) | code |  26.4% |     592 |
| connection (decode, reply, socket) | libc (memcpy, syscall wrappers) |   7.9% |     177 |
| connection (decode, reply, socket) | tokio mpsc |   0.6% |      13 |
| connection (decode, reply, socket) | tokio wake/schedule |   0.2% |       5 |
| **connection (decode, reply, socket)** | **all** | ** 35.1%** | **    788** |
| tokio scheduler | tokio wake/schedule |   2.2% |      49 |
| tokio scheduler | code |   0.8% |      18 |
| tokio scheduler | tokio io/split |   0.3% |       6 |
| tokio scheduler | libc (memcpy, syscall wrappers) |   0.1% |       1 |
| tokio scheduler | allocator |   0.0% |       0 |
| **tokio scheduler** | **all** | **  3.3%** | **     75** |
| AOF append/lead/gate | tokio io/split |   2.0% |      44 |
| AOF append/lead/gate | code |   0.6% |      14 |
| AOF append/lead/gate | libc (memcpy, syscall wrappers) |   0.5% |      11 |
| **AOF append/lead/gate** | **all** | **  3.1%** | **     69** |
| tokio io driver | code |   0.0% |       1 |
| tokio io driver | libc (memcpy, syscall wrappers) |   0.0% |       1 |
| **tokio io driver** | **all** | **  0.1%** | **      2** |

### Self (leaf)

| share | cycles/op | function |
|---:|---:|---|
|  19.6%    440.7 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|  12.3%    275.9 | `libc.so.6:+1c7919` |
|   8.0%    179.0 | `<core_engine::request::RequestDecoder>::decode_any` |
|   4.5%    102.0 | `[vdso]:+a8c` |
|   4.2%     94.2 | `recached_server::clients::record_metrics` |
|   4.0%     88.8 | `<core_engine::store::KeyValueStore>::set::<&str, &[u8]>` |
|   3.5%     78.1 | `<core_engine::store::KeyValueStore>::execute_ref_into` |
|   3.0%     67.7 | `core::str::converts::from_utf8` |
|   3.0%     66.4 | `core_engine::store::entry_size` |
|   2.7%     60.2 | `recached_server::propagation::try_execute_borrowed` |
|   2.6%     58.9 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|   2.3%     51.6 | `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` |
|   2.1%     46.6 | `core_engine::request::build_ref` |
|   1.9%     42.5 | `core_engine::store::lock_stripe` |
|   1.7%     38.8 | `<core_engine::store::SlotRef as core::ops::drop::Drop>::drop` |
|   1.4%     32.1 | `libc.so.6:+1c79c7` |
|   1.4%     31.0 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>>>::hash_u64::<&str>` |
|   1.1%     24.6 | `<std::hash::random::RandomState as core::hash::BuildHasher>::hash_one::<&str>` |
|   0.9%     20.3 | `<tokio::io::util::write_all::WriteAll<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::Ao…` |
|   0.9%     19.3 | `<std::sys::pal::unix::time::Timespec>::sub_timespec` |
|   0.8%     18.5 | `<core_engine::store::KeyIndex>::sync` |
|   0.8%     17.9 | `libc.so.6:+1c7d91` |
|   0.7%     15.8 | `core_engine::store::write_slot::<&str>` |
|   0.6%     13.4 | `recached_server::run::{closure#0}::{closure#27}::{closure#0}` |
|   0.6%     13.4 | `<std::sys::pal::unix::time::Timespec>::now` |
|   0.6%     13.4 | `<core_engine::request::Args>::key_ref` |
|   0.6%     12.6 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|   0.6%     12.6 | `<core_engine::resp::Value>::serialize_into` |
|   0.5%     12.0 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|   0.5%     11.8 | `[vdso]:+ac2` |
|   0.5%     10.7 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   0.5%     10.7 | `<recached_server::persistence::DeferredAof>::pop` |
|   0.4%      9.9 | `core::ptr::drop_glue::<core_engine::resp::Value>` |
|   0.4%      9.6 | `libc.so.6:+1c791d` |
|   0.4%      8.3 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|   0.4%      8.3 | `_rjem_malloc` |
|   0.4%      8.3 | `<tokio::sync::mpsc::list::Rx<()>>::pop` |
|   0.3%      7.8 | `[vdso]:+a8e` |
|   0.3%      6.2 | `_rjem_sdallocx` |
|   0.3%      5.9 | `<tokio::runtime::io::registration::Registration>::poll_ready` |
|   0.3%      5.6 | `[vdso]:+ac8` |
|   0.3%      5.6 | `<tokio::io::util::flush::Flush<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedW…` |
|   0.2%      5.4 | `libc.so.6:+1c7d80` |
|   0.2%      5.1 | `<tokio::sync::task::atomic_waker::AtomicWaker>::register_by_ref` |
|   0.2%      5.1 | `<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteH…` |
|   0.2%      4.8 | `<tokio::sync::mpsc::bounded::Receiver<()>>::recv::{closure#0}` |
|   0.2%      4.8 | `libc.so.6:+175818` |
|   0.2%      4.0 | `tokio::runtime::task::waker::wake_by_val` |
|   0.2%      3.7 | `[unknown]` |
|   0.2%      3.7 | `tokio::runtime::task::waker::clone_waker` |
|   0.2%      3.7 | `libc.so.6:+1c7e00` |
|   0.2%      3.7 | `[vdso]:+a73` |
|   0.2%      3.5 | `[vdso]:+a89` |
|   0.2%      3.5 | `<tokio::runtime::io::scheduled_io::ScheduledIo>::wake` |
|   0.2%      3.5 | `libc.so.6:+1c7e74` |
|   0.2%      3.5 | `libc.so.6:+139b40` |
|   0.2%      3.5 | `[vdso]:+a4f` |
|   0.1%      3.2 | `libc.so.6:+1c7e9f` |
|   0.1%      3.2 | `libc.so.6:+1c7d84` |
|   0.1%      3.2 | `libc.so.6:+1c792e` |

### Inclusive

| share | cycles/op | function |
|---:|---:|---|
| 100.0%   2244.9 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|  98.9%   2219.3 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|  98.3%   2206.4 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|  97.7%   2194.1 | `recached_server::run::{closure#0}::{closure#27}::{closure#0}` |
|  97.1%   2179.6 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|  74.0%   1661.6 | `recached_server::propagation::try_execute_borrowed` |
|  58.4%   1311.3 | `<core_engine::store::KeyValueStore>::execute_ref_into` |
|  49.3%   1106.8 | `<core_engine::store::KeyValueStore>::set::<&str, &[u8]>` |
|  39.4%    884.4 | `core_engine::store::write_slot::<&str>` |
|  35.9%    806.6 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|  14.6%    327.5 | `<core_engine::request::RequestDecoder>::decode_any` |
|  12.3%    275.9 | `libc.so.6:+1c7919` |
|  10.1%    227.5 | `recached_server::clients::record_metrics` |
|   8.4%    189.5 | `<std::sys::pal::unix::time::Timespec>::now` |
|   7.4%    166.7 | `libc.so.6:+139b5b` |
|   5.7%    127.6 | `core_engine::request::build_ref` |
|   4.5%    102.0 | `[vdso]:+a8c` |
|   3.6%     81.1 | `<core_engine::request::Args>::key_ref` |
|   3.0%     67.7 | `core::str::converts::from_utf8` |
|   3.0%     66.4 | `core_engine::store::entry_size` |
|   2.6%     57.3 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>>>::hash_u64::<&str>` |
|   2.3%     51.6 | `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` |
|   2.2%     49.5 | `<std::hash::random::RandomState as core::hash::BuildHasher>::hash_one::<&str>` |
|   2.2%     48.4 | `<core_engine::store::SlotRef as core::ops::drop::Drop>::drop` |
|   2.0%     45.0 | `<tokio::io::util::flush::Flush<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedW…` |
|   1.9%     42.5 | `core_engine::store::lock_stripe` |
|   1.7%     37.7 | `<core_engine::store::KeyIndex>::sync` |
|   1.5%     33.5 | `<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteH…` |
|   1.4%     32.1 | `libc.so.6:+1c79c7` |
|   1.3%     28.4 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   1.1%     24.6 | `<tokio::io::util::write_all::WriteAll<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::Ao…` |
|   1.0%     22.5 | `<tokio::sync::mpsc::bounded::Receiver<()>>::recv::{closure#0}` |
|   0.9%     19.3 | `<std::sys::pal::unix::time::Timespec>::sub_timespec` |
|   0.8%     17.9 | `libc.so.6:+1c7d91` |
|   0.8%     17.9 | `<core_engine::resp::Value>::serialize_into` |
|   0.7%     14.7 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::park_internal` |
|   0.6%     14.5 | `<tokio::net::tcp::stream::TcpStream>::poll_read_priv` |
|   0.6%     13.1 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   0.6%     12.8 | `<tokio::runtime::time::Driver>::park_internal` |
|   0.5%     11.8 | `[vdso]:+ac2` |
|   0.5%     11.2 | `<tokio::runtime::io::driver::Driver>::turn` |
|   0.5%     10.7 | `<recached_server::persistence::DeferredAof>::pop` |
|   0.5%     10.4 | `<tokio::net::tcp::stream::TcpStream>::poll_write_priv` |
|   0.4%      9.9 | `core::ptr::drop_glue::<core_engine::resp::Value>` |
|   0.4%      9.6 | `libc.so.6:+1c791d` |
|   0.4%      9.6 | `<tokio::runtime::io::scheduled_io::ScheduledIo>::wake` |
|   0.4%      9.4 | `<tokio::sync::task::atomic_waker::AtomicWaker>::register_by_ref` |
|   0.4%      8.3 | `std::sys::backtrace::__rust_begin_short_backtrace::<<tokio::runtime::blocking::pool::Spawner>::spawn_thread::…` |
|   0.4%      8.3 | `libc.so.6:+edc42` |
|   0.4%      8.3 | `<std::thread::lifecycle::spawn_unchecked<<tokio::runtime::blocking::pool::Spawner>::spawn_thread::{closure#0}…` |
|   0.4%      8.3 | `<std::sys::thread::unix::Thread>::new::thread_start` |
|   0.4%      8.3 | `libc.so.6:+17319c` |
|   0.4%      8.3 | `_rjem_malloc` |
|   0.4%      8.3 | `<tokio::sync::mpsc::list::Rx<()>>::pop` |
|   0.3%      7.8 | `[vdso]:+a8e` |
|   0.3%      7.0 | `<tokio::runtime::io::registration::Registration>::poll_ready` |
|   0.3%      6.2 | `_rjem_sdallocx` |
|   0.3%      6.2 | `tokio::runtime::task::waker::wake_by_val` |
|   0.3%      5.6 | `[vdso]:+ac8` |
|   0.2%      5.4 | `libc.so.6:+1c7d80` |
