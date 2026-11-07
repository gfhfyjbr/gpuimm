import argparse
import base64
import re
from pathlib import Path
from urllib.parse import unquote, urlsplit

from vncdotool import api
from vncdotool.client import VNCDoToolFactory


class KeyboardFactory(VNCDoToolFactory):
    force_caps = True


parser = argparse.ArgumentParser()
parser.add_argument('action', choices=['capture', 'run', 'keys'])
parser.add_argument('value')
parser.add_argument('--capture', default='glimm-windows-desktop.png')
args = parser.parse_args()
url = urlsplit(re.search(r'vnc://\S+', Path('glimm-tart-windows-run.log').read_text()).group())
client = api.connect(f'{url.hostname}::{url.port}', password=unquote(url.password), factory_class=KeyboardFactory, timeout=20)
try:
    if args.action == 'run':
        client.keyPress('esc')
        client.keyPress('ctrl-esc')
        for character in 'run':
            client.keyPress(character)
        client.pause(0.5)
        client.keyPress('enter')
        client.pause(0.5)
        client.keyPress('ctrl-a')
        for character in args.value:
            client.keyPress(character)
        client.keyPress('ctrl-shift-enter')
        client.pause(2)
    elif args.action == 'keys':
        for key in args.value.split(','):
            client.keyPress(key)
        client.pause(1)
    client.captureScreen(args.value if args.action == 'capture' else args.capture)
finally:
    client.disconnect()
