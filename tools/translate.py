#!/usr/bin/env python3
"""
translate.py - Bake translations into source files at build time.

Usage:
    python tools/translate.py [--lang LANG] SRC_DIR OUT_DIR

For each .py file in SRC_DIR:
  - Replaces t("key") calls with the translated literal string
  - Removes 'from lang import t' lines
  - Skips lang.py (source-only artifact, not deployed)

Translation lookup order: {lang}.json -> en.json -> key itself
"""

import argparse
import io
import json
import os
import re
import shutil
import tokenize


def load_translations(tools_dir, lang):
    trans_dir = os.path.join(tools_dir, 'translations')

    with open(os.path.join(trans_dir, 'en.json'), encoding='utf-8') as f:
        en = json.load(f)

    if lang == 'en':
        return en

    lang_path = os.path.join(trans_dir, f'{lang}.json')
    if not os.path.exists(lang_path):
        print(f"Warning: no translation file for '{lang}', falling back to English")
        return en

    with open(lang_path, encoding='utf-8') as f:
        lang_trans = json.load(f)

    merged = dict(en)
    merged.update(lang_trans)
    return merged


def _line_col_to_offset(source, pos):
    """Convert tokenize's (line, col) — 1-based line, 0-based col — to a char offset."""
    line, col = pos
    lines = source.splitlines(keepends=True)
    return sum(len(lines[i]) for i in range(line - 1)) + col


def translate_source(source, translations):
    """Replace t("key") calls with translated literals and strip the import."""
    # Strip 'from lang import t' lines
    source = re.sub(r'^from lang import t[ \t]*\r?\n', '', source, flags=re.MULTILINE)

    try:
        tokens = list(tokenize.generate_tokens(io.StringIO(source).readline))
    except tokenize.TokenError:
        return source

    _SKIP = {tokenize.NL, tokenize.NEWLINE, tokenize.COMMENT,
             tokenize.INDENT, tokenize.DEDENT}

    def next_real(start):
        i = start + 1
        while i < len(tokens) and tokens[i].type in _SKIP:
            i += 1
        return i

    replacements = []  # (start_offset, end_offset, new_text)
    i = 0
    while i < len(tokens):
        tok = tokens[i]
        if tok.type == tokenize.NAME and tok.string == 't':
            j = next_real(i)  # should be '('
            if j < len(tokens) and tokens[j].type == tokenize.OP and tokens[j].string == '(':
                k = next_real(j)  # should be the string literal
                if k < len(tokens) and tokens[k].type == tokenize.STRING:
                    m = next_real(k)  # should be ')'
                    if m < len(tokens) and tokens[m].type == tokenize.OP and tokens[m].string == ')':
                        try:
                            key = eval(tokens[k].string)
                        except Exception:
                            i += 1
                            continue
                        if isinstance(key, str):
                            translated = translations.get(key, key)
                            start = _line_col_to_offset(source, tok.start)
                            end   = _line_col_to_offset(source, tokens[m].end)
                            replacements.append((start, end, repr(translated)))
                            i = m + 1
                            continue
        i += 1

    for start, end, text in sorted(replacements, reverse=True):
        source = source[:start] + text + source[end:]

    return source


def translate_dir(src_dir, out_dir, translations):
    if os.path.exists(out_dir):
        # Preserve save.json if present so a rebuild doesn't wipe the save
        saved_save = None
        save_path = os.path.join(out_dir, 'save.json')
        if os.path.exists(save_path):
            with open(save_path, 'rb') as f:
                saved_save = f.read()
        shutil.rmtree(out_dir)
    else:
        saved_save = None

    translated = 0
    copied = 0

    for root, dirs, files in os.walk(src_dir):
        dirs[:] = [d for d in dirs if d != '__pycache__']
        rel = os.path.relpath(root, src_dir)
        dest_root = os.path.join(out_dir, rel)
        os.makedirs(dest_root, exist_ok=True)

        for fname in files:
            src_path  = os.path.join(root, fname)
            dest_path = os.path.join(dest_root, fname)

            if fname == 'lang.py':
                continue  # source-only, never deployed

            if fname.endswith('.py'):
                with open(src_path, encoding='utf-8') as f:
                    source = f.read()
                result = translate_source(source, translations)
                with open(dest_path, 'w', encoding='utf-8') as f:
                    f.write(result)
                translated += 1
            else:
                shutil.copy2(src_path, dest_path)
                copied += 1

    if saved_save is not None:
        with open(save_path, 'wb') as f:
            f.write(saved_save)

    print(f"[translate] {translated} .py files translated, {copied} other files copied -> {out_dir}")


def main():
    parser = argparse.ArgumentParser(description='Bake translations into source files.')
    parser.add_argument('--lang', default='en', help='Language code (default: en)')
    parser.add_argument('src', help='Source directory')
    parser.add_argument('out', help='Output directory')
    args = parser.parse_args()

    tools_dir = os.path.dirname(os.path.abspath(__file__))
    translations = load_translations(tools_dir, args.lang)
    translate_dir(args.src, args.out, translations)


if __name__ == '__main__':
    main()
