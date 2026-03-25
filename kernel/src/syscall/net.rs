use crate::syscall::result;

// Network syscalls - all stubbed pending a real socket layer.

const EAFNOSUPPORT: u64 = result::FAILURE;
const ENOSYS: u64 = result::FAILURE;

/// socket(domain, type, protocol) - Create socket (stub)
pub fn syscall_socket(_domain: i32, _sock_type: i32, _protocol: i32) -> u64 {
    EAFNOSUPPORT
}

/// bind(sockfd, addr, addrlen) - Bind socket (stub)
pub fn syscall_bind(_sockfd: i32, _addr: u64, _addrlen: u32) -> u64 {
    ENOSYS
}

/// connect(sockfd, addr, addrlen) - Connect socket (stub)
pub fn syscall_connect(_sockfd: i32, _addr: u64, _addrlen: u32) -> u64 {
    ENOSYS
}

/// listen(sockfd, backlog) - Listen on socket (stub)
pub fn syscall_listen(_sockfd: i32, _backlog: i32) -> u64 {
    ENOSYS
}

/// accept(sockfd, addr, addrlen) - Accept connection (stub)
pub fn syscall_accept(_sockfd: i32, _addr: u64, _addrlen: u64) -> u64 {
    ENOSYS
}

/// accept4(sockfd, addr, addrlen, flags) - Accept connection with flags (stub)
pub fn syscall_accept4(_sockfd: i32, _addr: u64, _addrlen: u64, _flags: i32) -> u64 {
    ENOSYS
}

/// sendto(sockfd, buf, len, flags, dest_addr, addrlen) - Send to socket (stub)
pub fn syscall_sendto(
    _sockfd: i32,
    _buf: u64,
    _len: u64,
    _flags: i32,
    _dest_addr: u64,
    _addrlen: u32,
) -> u64 {
    ENOSYS
}

/// recvfrom(sockfd, buf, len, flags, src_addr, addrlen) - Receive from socket (stub)
pub fn syscall_recvfrom(
    _sockfd: i32,
    _buf: u64,
    _len: u64,
    _flags: i32,
    _src_addr: u64,
    _addrlen: u64,
) -> u64 {
    ENOSYS
}

/// shutdown(sockfd, how) - Shut down socket (stub)
pub fn syscall_shutdown(_sockfd: i32, _how: i32) -> u64 {
    ENOSYS
}

/// setsockopt(sockfd, level, optname, optval, optlen) - Set socket option (stub)
pub fn syscall_setsockopt(
    _sockfd: i32,
    _level: i32,
    _optname: i32,
    _optval: u64,
    _optlen: u32,
) -> u64 {
    ENOSYS
}

/// getsockopt(sockfd, level, optname, optval, optlen) - Get socket option (stub)
pub fn syscall_getsockopt(
    _sockfd: i32,
    _level: i32,
    _optname: i32,
    _optval: u64,
    _optlen: u64,
) -> u64 {
    ENOSYS
}

/// getsockname(sockfd, addr, addrlen) - Get socket name (stub)
pub fn syscall_getsockname(_sockfd: i32, _addr: u64, _addrlen: u64) -> u64 {
    ENOSYS
}

/// getpeername(sockfd, addr, addrlen) - Get peer name (stub)
pub fn syscall_getpeername(_sockfd: i32, _addr: u64, _addrlen: u64) -> u64 {
    ENOSYS
}

/// socketpair(domain, type, protocol, sv) - Create socket pair (stub)
pub fn syscall_socketpair(_domain: i32, _sock_type: i32, _protocol: i32, _sv: u64) -> u64 {
    ENOSYS
}
