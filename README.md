# Weaving

A super easy to use static site generator, no config required (config is optional.)

Why "weaving?" I was looking for a static site generator that was well documented and predictable. Closest I found was Hugo and then I got to thinking- I haven't seen that film with Hugo Weaving in it where he wears a dress for ages.

Weaving.

/shrug

## Installation

> I'm working on distributing across different package managers.

Running `cargo install weaving` will install the `weaving` command to your system. 

## Usage

There are several commands you can use.

`weaving new -n my-site [-p path -t template-name]` will create a new folder with the specified template (only template that exists right now is `default`.)

`weaving build [-p path]` will build the weaving site at the specified (or default, current) working directory.

`weaving config [-p path -f force]` will generate a `weaving.toml` for you at the specified path. If you have one, you can overwrite with default using the `-f/--force` flag.

`weaving serve [-p path]` start a development server for the weaving site at the specified (or default current) path. Your site will be available at http://localhost:8080 by default (this can be chaged in `weaving.toml`)

### Building a site

Each piece of content must have at least these fields in it's frontmatter. Tags are used to give different pieces of content a way of relating to each other (naively.)

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

There is another filter built specifically for weaving `raw` which will dangerously output anything without any formatting or XSS protection. I don't recommend it's use anywhere other than outputting built in content.

### `weaving.toml`

All config is optional, the default config is this:

> NOTE image_config is currently unused but I plan to add image optimisation very soon

> NOTE npm_build is also unused, again I will be adding the ability to run a concurrent build command soon.

```toml
version = 1
content_dir = "content"
base_url = "localhost:8080"
partials_dir = "partials"
public_dir = "public"
build_dir = "site"
template_dir = "templates"
templating_language = "liquid"

[image_config]
quality = 83

[serve_config]
watch_excludes = [".git", "node_modules", "site"]
npm_build = false
address = "localhost:8080"
```


