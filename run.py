#!/usr/bin/env python3
"""
run.py - Translate and launch the Catode32 desktop emulator.

Usage:
    python run.py              # English (default)
    python run.py --lang nl    # Dutch
    python run.py --lang es    # Spanish (once es.json exists)
"""

import argparse
import os
import subprocess
import sys


def main():
    parser = argparse.ArgumentParser(description='Run Catode32 desktop emulator.')
    parser.add_argument('--lang', default='en', help='Language code (default: en)')
    parser.add_argument('--screenshot', action='store_true', help='Capture a screenshot after boot and save as screenshot.png')
    args = parser.parse_args()

    root    = os.path.dirname(os.path.abspath(__file__))
    src_dir = os.path.join(root, 'src')
    out_dir = os.path.join(root, 'build', f'desktop-{args.lang}')
    translate_script = os.path.join(root, 'tools', 'translate.py')
    main_script      = os.path.join(out_dir, 'main_desktop.py')

    result = subprocess.run(
        [sys.executable, translate_script, '--lang', args.lang, src_dir, out_dir]
    )
    if result.returncode != 0:
        sys.exit(result.returncode)

    # Pass src/ so config_desktop.py writes save.json there, not into build/
    env = os.environ.copy()
    env['CATODE32_SRC'] = src_dir

    if args.screenshot:
        env['CATODE32_SCREENSHOT'] = '1'
    os.execve(sys.executable, [sys.executable, main_script], env)


if __name__ == '__main__':
    main()
