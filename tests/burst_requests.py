#!/usr/bin/env python3
#
# Start a lot of requests to the server

from multiprocessing import Pool
import socket
import sys

HOST = "127.0.0.1"
PORT = 8000
BUFFER_SIZE = 1024


def connect2daemon(x):
    for res in socket.getaddrinfo(
            HOST, PORT, socket.AF_UNSPEC, socket.SOCK_STREAM):
        af, socktype, proto, canonname, sa = res
        try:
            s = socket.socket(af, socktype, proto)
        except socket.error as msg:
            s = None
            continue
        try:
            s.connect(sa)
        except socket.error as msg:
            s.close()
            s = None
            continue
        break

    if s is None:
        print('could not open socket')
        sys.exit(1)

    print("Connecting...")
    print("Ready.")
    client = s
    x = b"a 00:00:00:00:00:00"
    client.sendall(x)


p = Pool(processes=100)
print(p.map(connect2daemon, range(6000)))
