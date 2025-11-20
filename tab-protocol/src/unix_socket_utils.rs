use nix::sys::socket::{
	AddressFamily, Backlog, SockFlag, SockType, UnixAddr, accept, bind, connect, listen, socket,
};
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd, RawFd};
use std::os::unix::net::UnixStream;
use std::path::Path;
/// Bind a Unix seqpacket listener at the given path (removes any stale socket file).
pub fn bind_seqpacket_listener(path: impl AsRef<Path>) -> Result<RawFd, nix::Error> {
	let path = path.as_ref();
	let _ = std::fs::remove_file(path);

	let fd = socket(
		AddressFamily::Unix,
		SockType::SeqPacket,
		SockFlag::empty(),
		None,
	)?;
	let addr = UnixAddr::new(path)?;
	bind(fd.as_raw_fd(), &addr)?;
	listen(&fd, Backlog::new(16)?)?;
	Ok(fd.into_raw_fd())
}

/// Accept a seqpacket connection, returning it as a `UnixStream` for convenience.
pub fn accept_seqpacket(listener_fd: RawFd) -> Result<UnixStream, nix::Error> {
	let fd = accept(listener_fd)?;
	Ok(unsafe { UnixStream::from_raw_fd(fd.into_raw_fd()) })
}

/// Connect to a Unix seqpacket socket at the given path, returning it as a `UnixStream`.
pub fn connect_seqpacket(path: impl AsRef<Path>) -> Result<UnixStream, nix::Error> {
	let fd = socket(
		AddressFamily::Unix,
		SockType::Stream,
		SockFlag::empty(),
		None,
	)?;
	let addr = UnixAddr::new(path.as_ref())?;
	connect(fd.as_raw_fd(), &addr)?;
	Ok(unsafe { UnixStream::from_raw_fd(fd.into_raw_fd()) })
}
