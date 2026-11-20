samples: 18957, user cycles/op 5675

### Breakdown: innermost context x leaf class (each sample once)

| context | leaf class | share | cycles/op |
|---|---|---:|---:|
| connection (decode, reply, socket) | code |  25.0% |    1420 |
| connection (decode, reply, socket) | libc (memcpy, syscall wrappers) |   8.0% |     456 |
| connection (decode, reply, socket) | tokio mpsc |   3.5% |     196 |
| connection (decode, reply, socket) | tokio io/split |   3.1% |     177 |
| connection (decode, reply, socket) | tokio wake/schedule |   1.8% |     101 |
| **connection (decode, reply, socket)** | **all** | ** 41.4%** | **   2350** |
| store execute | code |  20.3% |    1150 |
| store execute | libc (memcpy, syscall wrappers) |   8.2% |     467 |
| store execute | allocator |   0.6% |      37 |
| **store execute** | **all** | ** 29.1%** | **   1654** |
| tokio scheduler | tokio wake/schedule |  11.4% |     648 |
| tokio scheduler | code |   3.3% |     185 |
| tokio scheduler | tokio io/split |   2.1% |     119 |
| tokio scheduler | libc (memcpy, syscall wrappers) |   0.2% |      13 |
| tokio scheduler | allocator |   0.0% |       1 |
| tokio scheduler | tokio mpsc |   0.0% |       0 |
| **tokio scheduler** | **all** | ** 17.0%** | **    967** |
| AOF append/lead/gate | code |   5.7% |     323 |
| AOF append/lead/gate | tokio io/split |   3.7% |     209 |
| AOF append/lead/gate | libc (memcpy, syscall wrappers) |   2.3% |     130 |
| **AOF append/lead/gate** | **all** | ** 11.7%** | **    662** |
| tokio io driver | code |   0.6% |      37 |
| tokio io driver | libc (memcpy, syscall wrappers) |   0.1% |       6 |
| **tokio io driver** | **all** | **  0.8%** | **     43** |

### Self (leaf)

| share | cycles/op | function |
|---:|---:|---|
|  12.4%    704.1 | `libc.so.6:_dl_mcount_wrapper@GLIBC_2.2.5` |
|   9.5%    537.7 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   7.7%    437.7 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|   6.3%    358.1 | `<core_engine::request::RequestDecoder>::decode_any` |
|   3.4%    193.7 | `<recached_server::persistence::DeferredAof>::pop` |
|   3.0%    168.0 | `recached_server::clients::record_metrics` |
|   2.6%    148.5 | `recached_server::run::{closure#0}::{closure#29}::{closure#0}` |
|   2.5%    143.4 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|   2.4%    138.0 | `recached_server::propagation::try_execute_borrowed` |
|   2.4%    136.2 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|   2.3%    130.8 | `<tokio::sync::mpsc::list::Rx<()>>::pop` |
|   2.1%    121.8 | `<tokio::io::split::ReadHalf<tokio::net::tcp::stream::TcpStream> as tokio::io::async_read::AsyncRead>::poll_re…` |
|   2.1%    119.5 | `<tokio::runtime::io::registration::Registration>::poll_ready` |
|   2.0%    112.3 | `libc.so.6:send@@GLIBC_2.2.5` |
|   1.9%    107.8 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|   1.8%    104.8 | `<core_engine::store::KeyValueStore>::execute_ref_into` |
|   1.8%    101.8 | `<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream> as tokio::io::async_write::AsyncWrite>::poll…` |
|   1.8%    101.5 | `<tokio::sync::task::atomic_waker::AtomicWaker>::register_by_ref` |
|   1.7%     98.2 | `[vdso]:+a8c` |
|   1.7%     96.4 | `<core_engine::store::KeyValueStore>::set::<&str, &[u8]>` |
|   1.5%     82.3 | `core_engine::request::build_ref` |
|   1.3%     74.5 | `<tokio::net::tcp::stream::TcpStream>::poll_write_priv` |
|   1.2%     67.4 | `core_engine::store::entry_size` |
|   1.1%     65.3 | `<tokio::sync::mpsc::bounded::Receiver<()>>::recv::{closure#0}` |
|   1.1%     65.0 | `tokio::runtime::task::waker::clone_waker` |
|   1.1%     64.1 | `<tokio::runtime::io::scheduled_io::ScheduledIo>::wake` |
|   1.1%     62.0 | `<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteH…` |
|   1.1%     61.7 | `<tokio::net::tcp::stream::TcpStream>::poll_read_priv` |
|   1.0%     56.6 | `[unknown]` |
|   1.0%     55.1 | `<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream> as tokio::io::async_write::AsyncWrite>::poll…` |
|   0.9%     53.9 | `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` |
|   0.9%     53.6 | `<core_engine::store::KeyIndex>::sync` |
|   0.9%     53.3 | `<core_engine::store::SlotRef as core::ops::drop::Drop>::drop` |
|   0.9%     50.9 | `tokio::runtime::task::waker::wake_by_val` |
|   0.9%     48.8 | `core::str::converts::from_utf8` |
|   0.8%     46.1 | `<core_engine::resp::Value>::serialize_into` |
|   0.7%     41.9 | `core_engine::store::lock_stripe` |
|   0.7%     37.7 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>>>::hash_u64::<&str>` |
|   0.6%     36.5 | `tokio::runtime::task::waker::drop_waker` |
|   0.6%     35.9 | `libc.so.6:clock_gettime@GLIBC_2.2.5` |

### Inclusive

| share | cycles/op | function |
|---:|---:|---|
| 100.0%   5675.3 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|  93.7%   5317.8 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|  91.1%   5172.9 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|  88.7%   5036.7 | `recached_server::run::{closure#0}::{closure#29}::{closure#0}` |
|  85.6%   4859.2 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|  38.6%   2189.3 | `recached_server::propagation::try_execute_borrowed` |
|  29.1%   1654.1 | `<core_engine::store::KeyValueStore>::execute_ref_into` |
|  22.4%   1273.8 | `<core_engine::store::KeyValueStore>::set::<&str, &[u8]>` |
|  18.0%   1020.9 | `core_engine::store::write_slot::<&str>` |
|  16.5%    936.1 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|  12.4%    704.1 | `libc.so.6:_dl_mcount_wrapper@GLIBC_2.2.5` |
|   9.5%    538.3 | `<core_engine::request::RequestDecoder>::decode_any` |
|   9.0%    510.4 | `<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteH…` |
|   6.7%    379.0 | `<tokio::sync::mpsc::bounded::Receiver<()>>::recv::{closure#0}` |
|   6.7%    377.5 | `<tokio::io::split::ReadHalf<tokio::net::tcp::stream::TcpStream> as tokio::io::async_read::AsyncRead>::poll_re…` |
|   5.9%    337.1 | `<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream> as tokio::io::async_write::AsyncWrite>::poll…` |
|   5.5%    311.4 | `recached_server::clients::record_metrics` |
|   4.5%    255.7 | `<tokio::net::tcp::stream::TcpStream>::poll_read_priv` |
|   4.1%    235.3 | `<tokio::net::tcp::stream::TcpStream>::poll_write_priv` |
|   3.9%    219.7 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   3.9%    219.4 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::park_internal` |
|   3.8%    213.8 | `<std::sys::pal::unix::time::Timespec>::now` |
|   3.6%    206.3 | `<tokio::runtime::time::Driver>::park_internal` |
|   3.5%    200.0 | `libc.so.6:clock_gettime@GLIBC_2.2.5` |
|   3.4%    193.7 | `<recached_server::persistence::DeferredAof>::pop` |
|   3.3%    190.1 | `<tokio::runtime::io::driver::Driver>::turn` |
|   3.2%    182.9 | `<tokio::sync::task::atomic_waker::AtomicWaker>::register_by_ref` |
|   2.6%    149.4 | `libc.so.6:send@@GLIBC_2.2.5` |
|   2.6%    147.3 | `<tokio::runtime::io::scheduled_io::ScheduledIo>::wake` |
|   2.5%    139.5 | `<tokio::runtime::io::registration::Registration>::poll_ready` |
|   2.5%    139.5 | `core_engine::request::build_ref` |
|   2.3%    130.8 | `<tokio::sync::mpsc::list::Rx<()>>::pop` |
|   2.3%    130.5 | `<core_engine::store::KeyIndex>::sync` |
|   1.9%    107.8 | `libc.so.6:clone@@GLIBC_2.2.5` |
|   1.9%    107.8 | `<std::sys::thread::unix::Thread>::new::thread_start` |
|   1.9%    107.8 | `libc.so.6:pthread_condattr_setpshared@GLIBC_2.2.5` |
|   1.9%    107.8 | `<std::thread::lifecycle::spawn_unchecked<<tokio::runtime::blocking::pool::Spawner>::spawn_thread::{closure#0}…` |
|   1.9%    107.8 | `std::sys::backtrace::__rust_begin_short_backtrace::<<tokio::runtime::blocking::pool::Spawner>::spawn_thread::…` |
|   1.7%     98.2 | `[vdso]:+a8c` |
|   1.5%     83.2 | `tokio::runtime::task::waker::wake_by_val` |
