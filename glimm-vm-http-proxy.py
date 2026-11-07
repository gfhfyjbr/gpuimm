"""Host-side HTTP proxy for task VMs when VPN fake-IP DNS is unreachable from vmnet."""
import argparse
import asyncio
import contextlib
import ipaddress
from urllib.parse import urlsplit


async def relay(reader, writer):
    try:
        while data := await reader.read(65536):
            writer.write(data)
            await writer.drain()
    finally:
        with contextlib.suppress(Exception):
            writer.write_eof()


async def handle(reader, writer):
    upstream = None
    try:
        header = await asyncio.wait_for(reader.readuntil(b"\r\n\r\n"), 30)
        lines = header.split(b"\r\n")
        method, target, version = lines[0].decode("ascii").split(" ", 2)
        if method == "CONNECT":
            host, port_text = target.rsplit(":", 1)
            port = int(port_text)
            request = None
        else:
            url = urlsplit(target)
            if url.scheme != "http" or not url.hostname:
                raise ValueError("expected an absolute HTTP URL")
            host, port = url.hostname, url.port or 80
            path = url.path or "/"
            if url.query:
                path += "?" + url.query
            lines[0] = f"{method} {path} {version}".encode("ascii")
            lines = [line for line in lines if not line.lower().startswith(b"proxy-connection:")]
            request = b"\r\n".join(lines)
        if port not in (80, 443) or "." not in host or host.lower().endswith((".local", ".localhost")):
            raise ValueError("only public HTTP(S) origins are allowed")
        try:
            ipaddress.ip_address(host)
        except ValueError:
            pass
        else:
            raise ValueError("literal destination IPs are not allowed")
        remote_reader, upstream = await asyncio.wait_for(asyncio.open_connection(host, port), 30)
        if request is None:
            writer.write(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            await writer.drain()
        else:
            upstream.write(request)
            await upstream.drain()
        await asyncio.gather(relay(reader, upstream), relay(remote_reader, writer))
    except Exception as error:
        print(type(error).__name__, str(error), flush=True)
        with contextlib.suppress(Exception):
            writer.write(b"HTTP/1.1 502 Bad Gateway\r\nConnection: close\r\nContent-Length: 0\r\n\r\n")
            await writer.drain()
    finally:
        writer.close()
        if upstream:
            upstream.close()
        with contextlib.suppress(Exception):
            await writer.wait_closed()


async def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", required=True)
    parser.add_argument("--port", type=int, default=51981)
    args = parser.parse_args()
    server = await asyncio.start_server(handle, args.host, args.port, limit=65536)
    print(f"VM HTTP proxy on {args.host}:{args.port}", flush=True)
    async with server:
        await server.serve_forever()


if __name__ == "__main__":
    asyncio.run(main())
