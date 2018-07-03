#!/usr/bin/env python3


import sys
import signal
import asyncio
import argparse
import time

import aiohttp


# =====
async def _run_client(loop: asyncio.AbstractEventLoop, url: str) -> None:
    def stdin_callback() -> None:
        line = sys.stdin.buffer.readline().decode()
        if line:
            asyncio.ensure_future(ws.send_str(line), loop=loop)
        else:
            loop.stop()

    loop.add_reader(sys.stdin.fileno(), stdin_callback)

    async def dispatch() -> None:
        while True:
            msg = await ws.receive()
            if msg.type == aiohttp.WSMsgType.TEXT:
                print("[%.5f] Received: %s" % (time.time(), msg.data.strip()))
            else:
                if msg.type == aiohttp.WSMsgType.CLOSE:
                    await ws.close()
                elif msg.type == aiohttp.WSMsgType.ERROR:
                    print("[%.5f] Error during receive: %s" % (time.time(), ws.exception()))
                elif msg.type == aiohttp.WSMsgType.CLOSED:
                    pass
                break

    async with aiohttp.ClientSession().ws_connect(url) as ws:
        await dispatch()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("-u", "--url", default="http://localhost:8081/ws")
    options = parser.parse_args()

    loop = asyncio.get_event_loop()
    loop.add_signal_handler(signal.SIGINT, loop.stop)
    loop.create_task(_run_client(loop, options.url))
    loop.run_forever()
