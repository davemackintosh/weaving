# Weaving

A static site generator that works by running a command, no config (config is optional.)

Markdown, Liquid -> HTML, CSS out.

Simple.

Why weaving? I was looking for a static site generator that was well documented and predictable. Closest I found was Hugo and then I got to thinking- I haven't seen that film with Hugo Weaving in it where he wears a dress for ages.

Weaving.

/shrug

## Why are there so many unwrap and some panic!()?

Well, frankly? This is for my blog and some passion projects. If other people like the idea of a config-less static site generator, I'm be super grateful for more patient folks to submit PRs improving it.

## Installation

Currently, installation is done by building it yourself but I will get around to packaging it and distributing via binstall, etc.

Clone this repo and then run `cargo install --path ./crates/weaving` and you'll get yourself a `weaving` command.

### Usage

If you're in a suitable folder, you can run `weaving build`

Where you run that you should have the following folder structure

```
./
  ./content <-- this is where your markdown goes
  ./templates <-- this is where your liquid templates go.
    default.liquid <-- this is required if any of your content doesn't specify a template in it's frontmatter.
```

### Building a site

When you're developing a site, it's useful to see it in your browser easily. You can run `weaving serve` to create a simple web server.

Content must have at least these fields in it's frontmatter

```
---
title: test
tags:
  - test
---
```

You can put any other keys you like in there and they will be available in your liquid templates as `page.user.YOUR_KEY`

The page object in your liquid templates has these possible keys:

```
title: String
tags: Array<String>
keywords: Array<String>?
description: String?
user: Map<String, any>
```

The built in filters in liquid templates are:

```
abs, append, at_least, at_most, capitalize, ceil, compact, concat, date, default, divided_by, downcase, escape, escape_once, first, floor, join, last, lstrip, map, minus, modulo, newline_to_br, plus, prepend, raw, remove, remove_first, replace, replace_first, reverse, round, rstrip, size, slice, sort, sort_natural, split, strip, strip_html, strip_newlines, times, truncate, truncatewords, uniq, upcase, url_decode, url_encode, where
```

There is another filter built specifically for weaving `raw` which will dangerously output anything without any formatting or XSS protection.
