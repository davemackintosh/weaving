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

Clone this repo and then run `cargo build --release` and you'll get yourself a `weaving` binary in `target/release/weaving` which you can move to somewhere on your `PATH`.

### Usage

`weaving build -c .`

Where you run that you should have the following folder structure

```
./
  ./content <-- this is where your markdown goes
  ./templates <-- this is where your liquid templates go.
    default.liquid <-- this is required if any of your content doesn't specify a template in it's frontmatter.
```

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

The available filters in liquid templates are:

```
TBD
```
