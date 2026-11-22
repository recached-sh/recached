samples: 19756, user cycles/op 6199

### Stack contains (inclusive)

| share | cycles/op | label |
|---:|---:|---|
|   9.1%    564.8 | `decode (all)` |
|   0.0%      0.0 | `decode: general Value path (Args::general)` |
|   0.0%      0.0 | `decode: from_value` |
|   0.0%      0.0 | `decode: build (owned MSET/DEL)` |
|   0.0%      0.0 | `to_command (borrowed -> owned)` |
|  40.4%   2504.6 | `borrowed execute (try_execute_borrowed)` |
|   0.0%      0.0 | `ordered write entry (execute_ordered_write)` |
|  32.8%   2035.2 | `store execute (all)` |
|   0.0%      0.0 | `store: Command clone` |
|   2.2%    136.2 | `store: key hash (sip)` |
|   0.0%      0.0 | `store: entry_state` |
|   0.0%      0.0 | `store: extract_key` |
|   3.7%    228.1 | `clock reads` |
|   4.5%    279.6 | `metrics record` |
|   0.6%     38.6 | `reply serialize (Value)` |
|   0.5%     32.6 | `Value drop` |
|   0.0%      0.0 | `Command drop` |
|   1.5%     92.3 | `allocator (all)` |
|   7.4%    460.3 | `select overflow arm (mpsc recv)` |
|  12.4%    768.4 | `BufWriter flush (all)` |
|  13.0%    806.1 | `AofGatedWriter (all)` |
|   2.6%    161.0 | `DeferredAof pop` |
|   4.1%    251.3 | `socket read (poll_read_priv)` |
|   3.4%    211.2 | `socket write (poll_write_priv)` |
|  13.0%    806.4 | `split lock (ReadHalf/WriteHalf)` |
|   4.0%    246.6 | `tokio park/driver` |
|  94.0%   5829.7 | `tokio run_task (scheduler)` |
|   0.0%      0.0 | `task-local scope (with_deferred_aof)` |

### Breakdown: innermost context x leaf class (each sample once)

| context | leaf class | share | cycles/op |
|---|---|---:|---:|
| connection (decode, reply, socket) | code |  24.9% |    1544 |
| connection (decode, reply, socket) | libc (memcpy, syscall wrappers) |   6.3% |     389 |
| connection (decode, reply, socket) | tokio mpsc |   3.8% |     234 |
| connection (decode, reply, socket) | tokio wake/schedule |   2.2% |     134 |
| **connection (decode, reply, socket)** | **all** | ** 37.1%** | **   2301** |
| store execute | code |  23.2% |    1435 |
| store execute | libc (memcpy, syscall wrappers) |   8.2% |     509 |
| store execute | allocator |   1.5% |      91 |
| **store execute** | **all** | ** 32.8%** | **   2035** |
| tokio scheduler | tokio wake/schedule |  11.4% |     709 |
| tokio scheduler | code |   2.6% |     164 |
| tokio scheduler | tokio io/split |   2.0% |     123 |
| tokio scheduler | libc (memcpy, syscall wrappers) |   0.3% |      18 |
| tokio scheduler | allocator |   0.0% |       1 |
| **tokio scheduler** | **all** | ** 16.4%** | **   1015** |
| AOF append/lead/gate | tokio io/split |   6.8% |     421 |
| AOF append/lead/gate | code |   4.2% |     262 |
| AOF append/lead/gate | libc (memcpy, syscall wrappers) |   2.0% |     122 |
| **AOF append/lead/gate** | **all** | ** 13.0%** | **    805** |
| tokio io driver | code |   0.6% |      38 |
| tokio io driver | libc (memcpy, syscall wrappers) |   0.1% |       4 |
| **tokio io driver** | **all** | **  0.7%** | **     43** |

### Self (leaf)

| share | cycles/op | function |
|---:|---:|---|
|  10.9%    673.1 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   9.8%    607.2 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|   5.5%    339.5 | `libc.so.6:+1c7919` |
|   3.8%    237.5 | `<core_engine::request::RequestDecoder>::decode_any` |
|   2.7%    166.0 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   2.6%    164.1 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|   2.6%    161.0 | `<recached_server::persistence::DeferredAof>::pop` |
|   2.5%    155.9 | `<tokio::sync::mpsc::list::Rx<()>>::pop` |
|   2.4%    147.5 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|   2.3%    144.7 | `recached_server::clients::record_metrics` |
|   2.2%    136.8 | `libc.so.6:+1c7d91` |
|   2.2%    133.7 | `<tokio::sync::task::atomic_waker::AtomicWaker>::register_by_ref` |
|   2.1%    130.8 | `core::str::converts::from_utf8` |
|   2.0%    124.3 | `recached_server::run::{closure#0}::{closure#27}::{closure#0}` |
|   2.0%    122.7 | `<tokio::runtime::io::registration::Registration>::poll_ready` |
|   1.9%    118.6 | `<core_engine::store::KeyValueStore>::execute_ref_into` |
|   1.9%    118.3 | `core_engine::request::build_ref` |
|   1.9%    114.8 | `<core_engine::store::KeyValueStore>::set::<&str, &[u8]>` |
|   1.7%    107.3 | `[vdso]:+a8c` |
|   1.7%    104.8 | `recached_server::propagation::try_execute_borrowed` |
|   1.6%     97.0 | `<tokio::io::util::flush::Flush<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedW…` |
|   1.5%     95.7 | `tokio::runtime::task::waker::clone_waker` |
|   1.5%     94.4 | `core_engine::store::lock_stripe` |
|   1.4%     88.5 | `<core_engine::store::SlotRef as core::ops::drop::Drop>::drop` |
|   1.4%     86.9 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|   1.3%     77.8 | `<tokio::sync::mpsc::bounded::Receiver<()>>::recv::{closure#0}` |
|   1.3%     77.5 | `<tokio::runtime::io::scheduled_io::ScheduledIo>::wake` |
|   1.2%     74.4 | `<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteH…` |
|   1.2%     72.8 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>>>::hash_u64::<&str>` |
|   1.0%     63.4 | `_rjem_malloc` |
|   1.0%     63.4 | `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` |
|   1.0%     62.4 | `<tokio::net::tcp::stream::TcpStream>::poll_write_priv` |
|   1.0%     60.6 | `core_engine::store::entry_size` |
|   0.9%     58.7 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   0.9%     55.5 | `tokio::runtime::task::waker::wake_by_val` |
|   0.9%     54.9 | `<tokio::net::tcp::stream::TcpStream>::poll_read_priv` |
|   0.9%     54.3 | `<core_engine::store::KeyIndex>::sync` |
|   0.8%     50.5 | `libc.so.6:+1c79c7` |
|   0.8%     48.9 | `<core_engine::request::Args>::key_ref` |
|   0.8%     46.8 | `[unknown]` |
|   0.7%     45.2 | `libc.so.6:+175818` |
|   0.6%     36.4 | `<tokio::runtime::io::driver::Driver>::turn` |
|   0.5%     32.6 | `core::ptr::drop_glue::<core_engine::resp::Value>` |
|   0.5%     30.1 | `tokio::runtime::task::waker::drop_waker` |
|   0.5%     28.9 | `<core_engine::resp::Value>::serialize_into` |
|   0.5%     28.6 | `<std::hash::random::RandomState as core::hash::BuildHasher>::hash_one::<&str>` |
|   0.4%     26.0 | `<std::sys::pal::unix::time::Timespec>::sub_timespec` |
|   0.4%     25.4 | `<tokio::io::util::write_all::WriteAll<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::Ao…` |
|   0.4%     24.5 | `libc.so.6:+1c7e47` |
|   0.3%     21.7 | `_rjem_sdallocx` |
|   0.3%     21.0 | `<tokio::runtime::scheduler::multi_thread::handle::Handle>::schedule_task::{closure#0}` |
|   0.3%     18.2 | `libc.so.6:+175826` |
|   0.3%     17.3 | `core_engine::store::write_slot::<&str>` |
|   0.3%     17.3 | `<recached_server::persistence::AOF_DEFERRED::__KEY::{K#0}::{closure#0} as core::ops::function::FnOnce<(core::…` |
|   0.3%     17.3 | `libc.so.6:+17581c` |
|   0.2%     14.4 | `libc.so.6:+1c791d` |
|   0.2%     13.8 | `libc.so.6:+1754b8` |
|   0.2%     13.8 | `libc.so.6:+f6332` |
|   0.2%     12.2 | `<std::sys::pal::unix::time::Timespec>::now` |
|   0.2%     11.3 | `[vdso]:+ac2` |

### Inclusive

| share | cycles/op | function |
|---:|---:|---|
| 100.0%   6199.0 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|  94.0%   5829.7 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|  91.4%   5663.4 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|  89.0%   5515.9 | `recached_server::run::{closure#0}::{closure#27}::{closure#0}` |
|  86.7%   5372.5 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|  40.4%   2504.6 | `recached_server::propagation::try_execute_borrowed` |
|  32.8%   2035.2 | `<core_engine::store::KeyValueStore>::execute_ref_into` |
|  25.6%   1587.4 | `<core_engine::store::KeyValueStore>::set::<&str, &[u8]>` |
|  19.8%   1225.0 | `core_engine::store::write_slot::<&str>` |
|  18.4%   1140.3 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|  12.4%    768.4 | `<tokio::io::util::flush::Flush<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedW…` |
|   9.1%    564.8 | `<core_engine::request::RequestDecoder>::decode_any` |
|   8.9%    554.8 | `<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteH…` |
|   7.7%    480.4 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   7.4%    460.3 | `<tokio::sync::mpsc::bounded::Receiver<()>>::recv::{closure#0}` |
|   5.5%    339.5 | `libc.so.6:+1c7919` |
|   4.8%    298.1 | `core_engine::request::build_ref` |
|   4.5%    279.6 | `recached_server::clients::record_metrics` |
|   4.1%    251.3 | `<tokio::net::tcp::stream::TcpStream>::poll_read_priv` |
|   4.0%    246.6 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::park_internal` |
|   3.7%    229.7 | `<tokio::runtime::time::Driver>::park_internal` |
|   3.7%    228.1 | `<std::sys::pal::unix::time::Timespec>::now` |
|   3.6%    226.2 | `<tokio::sync::task::atomic_waker::AtomicWaker>::register_by_ref` |
|   3.5%    219.6 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   3.4%    211.2 | `<tokio::net::tcp::stream::TcpStream>::poll_write_priv` |
|   3.4%    210.9 | `<tokio::runtime::io::driver::Driver>::turn` |
|   3.1%    193.3 | `libc.so.6:+139b5b` |
|   2.9%    179.8 | `<core_engine::request::Args>::key_ref` |
|   2.7%    168.2 | `<tokio::runtime::io::scheduled_io::ScheduledIo>::wake` |
|   2.6%    161.0 | `<recached_server::persistence::DeferredAof>::pop` |
|   2.5%    155.9 | `<tokio::runtime::io::registration::Registration>::poll_ready` |
|   2.5%    155.9 | `<tokio::sync::mpsc::list::Rx<()>>::pop` |
|   2.2%    136.8 | `libc.so.6:+1c7d91` |
|   2.1%    133.0 | `<core_engine::store::KeyIndex>::sync` |
|   2.1%    130.8 | `core::str::converts::from_utf8` |
|   1.8%    113.9 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>>>::hash_u64::<&str>` |
|   1.7%    107.3 | `[vdso]:+a8c` |
|   1.5%     95.7 | `<core_engine::store::SlotRef as core::ops::drop::Drop>::drop` |
|   1.5%     95.7 | `tokio::runtime::task::waker::clone_waker` |
|   1.5%     94.4 | `core_engine::store::lock_stripe` |
|   1.5%     90.7 | `tokio::runtime::task::waker::wake_by_val` |
|   1.4%     86.9 | `libc.so.6:+edc42` |
|   1.4%     86.9 | `<std::thread::lifecycle::spawn_unchecked<<tokio::runtime::blocking::pool::Spawner>::spawn_thread::{closure#0}…` |
|   1.4%     86.9 | `libc.so.6:+17319c` |
|   1.4%     86.9 | `std::sys::backtrace::__rust_begin_short_backtrace::<<tokio::runtime::blocking::pool::Spawner>::spawn_thread::…` |
|   1.4%     86.9 | `<std::sys::thread::unix::Thread>::new::thread_start` |
|   1.0%     63.4 | `_rjem_malloc` |
|   1.0%     63.4 | `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` |
|   1.0%     60.6 | `core_engine::store::entry_size` |
|   0.8%     50.5 | `libc.so.6:+1c79c7` |
|   0.8%     50.2 | `<std::hash::random::RandomState as core::hash::BuildHasher>::hash_one::<&str>` |
|   0.8%     46.8 | `[unknown]` |
|   0.7%     45.2 | `libc.so.6:+175818` |
|   0.6%     38.6 | `<core_engine::resp::Value>::serialize_into` |
|   0.6%     37.7 | `<tokio::io::util::write_all::WriteAll<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::Ao…` |
|   0.5%     32.6 | `libc.so.6:+1754b3` |
|   0.5%     32.6 | `core::ptr::drop_glue::<core_engine::resp::Value>` |
|   0.5%     32.3 | `libc.so.6:+175813` |
|   0.5%     30.4 | `<tokio::runtime::scheduler::multi_thread::handle::Handle>::schedule_task` |
|   0.5%     30.1 | `tokio::runtime::task::waker::drop_waker` |
