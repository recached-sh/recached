samples: 19389, user cycles/op 6226

### Stack contains (inclusive)

| share | cycles/op | label |
|---:|---:|---|
|  11.0%    682.1 | `decode (all)` |
|   0.0%      0.0 | `decode: general Value path (Args::general)` |
|   0.0%      0.0 | `decode: from_value` |
|   0.0%      0.0 | `decode: build (owned MSET/DEL)` |
|   0.0%      0.0 | `to_command (borrowed -> owned)` |
|  41.9%   2610.4 | `borrowed execute (try_execute_borrowed)` |
|   0.0%      0.0 | `ordered write entry (execute_ordered_write)` |
|  33.2%   2068.3 | `store execute (all)` |
|   0.0%      0.0 | `store: Command clone` |
|   2.7%    169.2 | `store: key hash (sip)` |
|   0.0%      0.0 | `store: entry_state` |
|   0.0%      0.0 | `store: extract_key` |
|   4.4%    273.3 | `clock reads` |
|   4.6%    285.5 | `metrics record` |
|   0.4%     22.8 | `reply serialize (Value)` |
|   0.3%     19.9 | `Value drop` |
|   0.0%      0.0 | `Command drop` |
|   0.2%     13.5 | `allocator (all)` |
|   6.8%    424.2 | `select overflow arm (mpsc recv)` |
|  10.6%    662.5 | `BufWriter flush (all)` |
|  11.4%    709.0 | `AofGatedWriter (all)` |
|   2.6%    163.1 | `DeferredAof pop` |
|   4.0%    246.6 | `socket read (poll_read_priv)` |
|   3.5%    215.5 | `socket write (poll_write_priv)` |
|  11.4%    709.0 | `split lock (ReadHalf/WriteHalf)` |
|   3.5%    217.1 | `tokio park/driver` |
|  94.5%   5882.9 | `tokio run_task (scheduler)` |
|   0.0%      0.0 | `task-local scope (with_deferred_aof)` |

### Breakdown: innermost context x leaf class (each sample once)

| context | leaf class | share | cycles/op |
|---|---|---:|---:|
| connection (decode, reply, socket) | code |  25.4% |    1583 |
| connection (decode, reply, socket) | libc (memcpy, syscall wrappers) |   7.3% |     457 |
| connection (decode, reply, socket) | tokio mpsc |   3.7% |     232 |
| connection (decode, reply, socket) | tokio wake/schedule |   1.8% |     111 |
| **connection (decode, reply, socket)** | **all** | ** 38.3%** | **   2384** |
| store execute | code |  25.1% |    1561 |
| store execute | libc (memcpy, syscall wrappers) |   7.9% |     495 |
| store execute | allocator |   0.2% |      13 |
| **store execute** | **all** | ** 33.2%** | **   2068** |
| tokio scheduler | tokio wake/schedule |  11.6% |     722 |
| tokio scheduler | code |   2.4% |     150 |
| tokio scheduler | tokio io/split |   2.0% |     122 |
| tokio scheduler | libc (memcpy, syscall wrappers) |   0.3% |      19 |
| tokio scheduler | allocator |   0.0% |       1 |
| tokio scheduler | tokio mpsc |   0.0% |       0 |
| **tokio scheduler** | **all** | ** 16.3%** | **   1014** |
| AOF append/lead/gate | tokio io/split |   5.1% |     315 |
| AOF append/lead/gate | code |   4.6% |     289 |
| AOF append/lead/gate | libc (memcpy, syscall wrappers) |   1.9% |     118 |
| **AOF append/lead/gate** | **all** | ** 11.6%** | **    722** |
| tokio io driver | code |   0.5% |      32 |
| tokio io driver | libc (memcpy, syscall wrappers) |   0.1% |       6 |
| **tokio io driver** | **all** | **  0.6%** | **     38** |

### Self (leaf)

| share | cycles/op | function |
|---:|---:|---|
|  10.8%    672.4 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   8.4%    525.4 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|   6.0%    375.4 | `<core_engine::request::RequestDecoder>::decode_any` |
|   5.1%    319.8 | `libc.so.6:+1c7919` |
|   2.9%    183.4 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|   2.8%    173.4 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|   2.6%    163.1 | `<recached_server::persistence::DeferredAof>::pop` |
|   2.5%    155.1 | `libc.so.6:+1c7d91` |
|   2.4%    147.7 | `<tokio::sync::mpsc::list::Rx<()>>::pop` |
|   2.3%    144.8 | `<core_engine::store::KeyValueStore>::execute_ref_into` |
|   2.3%    141.0 | `recached_server::clients::record_metrics` |
|   2.0%    127.2 | `recached_server::propagation::try_execute_borrowed` |
|   2.0%    122.0 | `<tokio::runtime::io::registration::Registration>::poll_ready` |
|   1.9%    115.9 | `[vdso]:+a8c` |
|   1.8%    115.0 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   1.8%    112.7 | `recached_server::run::{closure#0}::{closure#27}::{closure#0}` |
|   1.8%    111.1 | `<tokio::sync::task::atomic_waker::AtomicWaker>::register_by_ref` |
|   1.8%    109.5 | `core_engine::store::incr_by::<&str>` |
|   1.6%    101.8 | `<core_engine::request::Args>::key_ref` |
|   1.6%     99.9 | `<core_engine::store::SlotRef as core::ops::drop::Drop>::drop` |
|   1.5%     93.8 | `core::str::converts::from_utf8` |
|   1.5%     91.8 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>>>::hash_u64::<&str>` |
|   1.5%     91.2 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|   1.4%     84.8 | `<tokio::sync::mpsc::bounded::Receiver<()>>::recv::{closure#0}` |
|   1.3%     82.5 | `<tokio::io::util::flush::Flush<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedW…` |
|   1.2%     77.4 | `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` |
|   1.2%     73.9 | `core_engine::store::lock_stripe` |
|   1.2%     73.9 | `tokio::runtime::task::waker::clone_waker` |
|   1.2%     73.5 | `<tokio::net::tcp::stream::TcpStream>::poll_write_priv` |
|   1.0%     64.5 | `<tokio::runtime::io::scheduled_io::ScheduledIo>::wake` |
|   1.0%     60.7 | `core_engine::store::entry_size` |
|   0.9%     58.4 | `core_engine::request::build_ref` |
|   0.9%     54.9 | `libc.so.6:+175818` |
|   0.9%     54.6 | `<core_engine::store::KeyIndex>::sync` |
|   0.9%     54.6 | `<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteH…` |
|   0.8%     51.1 | `<tokio::net::tcp::stream::TcpStream>::poll_read_priv` |
|   0.8%     50.1 | `tokio::runtime::task::waker::wake_by_val` |
|   0.8%     48.5 | `[unknown]` |
|   0.7%     45.6 | `libc.so.6:+1c79c7` |
|   0.6%     39.8 | `tokio::runtime::task::waker::drop_waker` |
|   0.6%     35.6 | `core_engine::store::write_slot::<&str>` |
|   0.6%     35.6 | `<core_engine::resp::Value>::parse_inner` |
|   0.6%     34.4 | `<std::sys::pal::unix::time::Timespec>::sub_timespec` |
|   0.5%     31.5 | `<tokio::runtime::io::driver::Driver>::turn` |
|   0.5%     31.5 | `<tokio::io::util::write_all::WriteAll<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::Ao…` |
|   0.5%     31.1 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   0.5%     30.5 | `<recached_server::persistence::AOF_DEFERRED::__KEY::{K#0}::{closure#0} as core::ops::function::FnOnce<(core::…` |
|   0.5%     30.2 | `<std::hash::random::RandomState as core::hash::BuildHasher>::hash_one::<&str>` |
|   0.4%     22.8 | `<core_engine::resp::Value>::serialize_into` |
|   0.3%     20.9 | `core_engine::resp::format_i64` |
|   0.3%     20.6 | `libc.so.6:+139b52` |
|   0.3%     19.9 | `core::ptr::drop_glue::<core_engine::resp::Value>` |
|   0.3%     19.3 | `libc.so.6:+1c7e47` |
|   0.3%     19.3 | `<tokio::runtime::scheduler::multi_thread::handle::Handle>::schedule_task::{closure#0}` |
|   0.3%     18.9 | `core_engine::resp::write_header` |
|   0.3%     16.4 | `libc.so.6:+17581c` |
|   0.3%     16.4 | `<core_engine::store::StrValue>::parse_as::<i64>` |
|   0.3%     15.7 | `libc.so.6:+1c791d` |
|   0.2%     14.5 | `libc.so.6:+1c7ddf` |
|   0.2%     14.1 | `libc.so.6:+f6332` |

### Inclusive

| share | cycles/op | function |
|---:|---:|---|
| 100.0%   6226.2 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|  94.5%   5882.9 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|  91.7%   5707.6 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|  88.7%   5523.6 | `recached_server::run::{closure#0}::{closure#27}::{closure#0}` |
|  86.4%   5379.4 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|  41.9%   2610.4 | `recached_server::propagation::try_execute_borrowed` |
|  33.2%   2068.3 | `<core_engine::store::KeyValueStore>::execute_ref_into` |
|  26.0%   1618.5 | `core_engine::store::incr_by::<&str>` |
|  20.2%   1258.5 | `core_engine::store::write_slot::<&str>` |
|  18.7%   1161.5 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|  11.0%    682.1 | `<core_engine::request::RequestDecoder>::decode_any` |
|  10.6%    662.5 | `<tokio::io::util::flush::Flush<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedW…` |
|   7.9%    489.1 | `<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteH…` |
|   7.0%    434.2 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   6.8%    424.2 | `<tokio::sync::mpsc::bounded::Receiver<()>>::recv::{closure#0}` |
|   5.1%    319.8 | `libc.so.6:+1c7919` |
|   4.6%    285.5 | `recached_server::clients::record_metrics` |
|   4.4%    273.3 | `<std::sys::pal::unix::time::Timespec>::now` |
|   4.0%    246.6 | `<tokio::net::tcp::stream::TcpStream>::poll_read_priv` |
|   3.8%    237.3 | `core_engine::request::build_ref` |
|   3.5%    217.1 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::park_internal` |
|   3.5%    215.5 | `<tokio::net::tcp::stream::TcpStream>::poll_write_priv` |
|   3.4%    211.9 | `libc.so.6:+139b5b` |
|   3.3%    203.9 | `<tokio::runtime::time::Driver>::park_internal` |
|   3.1%    194.3 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   3.1%    191.7 | `<tokio::sync::task::atomic_waker::AtomicWaker>::register_by_ref` |
|   3.0%    184.3 | `<tokio::runtime::io::driver::Driver>::turn` |
|   2.9%    178.9 | `<core_engine::request::Args>::key_ref` |
|   2.6%    163.1 | `<recached_server::persistence::DeferredAof>::pop` |
|   2.5%    155.1 | `libc.so.6:+1c7d91` |
|   2.5%    155.1 | `<tokio::runtime::io::registration::Registration>::poll_ready` |
|   2.4%    150.0 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>>>::hash_u64::<&str>` |
|   2.4%    147.7 | `<tokio::sync::mpsc::list::Rx<()>>::pop` |
|   2.3%    146.1 | `<tokio::runtime::io::scheduled_io::ScheduledIo>::wake` |
|   2.1%    132.3 | `<core_engine::store::KeyIndex>::sync` |
|   1.9%    115.9 | `[vdso]:+a8c` |
|   1.8%    110.8 | `<core_engine::store::SlotRef as core::ops::drop::Drop>::drop` |
|   1.5%     93.8 | `core::str::converts::from_utf8` |
|   1.5%     91.2 | `libc.so.6:+17319c` |
|   1.5%     91.2 | `libc.so.6:+edc42` |
|   1.5%     91.2 | `std::sys::backtrace::__rust_begin_short_backtrace::<<tokio::runtime::blocking::pool::Spawner>::spawn_thread::…` |
|   1.5%     91.2 | `<std::thread::lifecycle::spawn_unchecked<<tokio::runtime::blocking::pool::Spawner>::spawn_thread::{closure#0}…` |
|   1.5%     91.2 | `<std::sys::thread::unix::Thread>::new::thread_start` |
|   1.3%     81.6 | `tokio::runtime::task::waker::wake_by_val` |
|   1.2%     77.4 | `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` |
|   1.2%     73.9 | `core_engine::store::lock_stripe` |
|   1.2%     73.9 | `tokio::runtime::task::waker::clone_waker` |
|   1.0%     60.7 | `core_engine::store::entry_size` |
|   0.9%     54.9 | `libc.so.6:+175818` |
|   0.8%     48.5 | `[unknown]` |
|   0.8%     47.8 | `<std::hash::random::RandomState as core::hash::BuildHasher>::hash_one::<&str>` |
|   0.7%     46.6 | `<tokio::io::util::write_all::WriteAll<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::Ao…` |
|   0.7%     45.6 | `libc.so.6:+1c79c7` |
|   0.7%     41.1 | `core_engine::resp::write_header` |
|   0.6%     39.8 | `tokio::runtime::task::waker::drop_waker` |
|   0.6%     35.6 | `<core_engine::resp::Value>::parse_inner` |
|   0.6%     35.0 | `libc.so.6:+175813` |
|   0.6%     34.4 | `<std::sys::pal::unix::time::Timespec>::sub_timespec` |
|   0.6%     34.4 | `libc.so.6:+1754b3` |
|   0.5%     33.1 | `<core_engine::store::StrValue>::parse_as::<i64>` |
