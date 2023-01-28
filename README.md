# actix-drop

actix-drop is a simple web app for sharing texts across different computers.

Most of my computers run on different Linux distros, my servers on OpenBSD,
and my phone is iOS, which make it super difficult to share clipboards.

I could have used ready-made solution like PasteBin or email the text to myself,
but that would make me nervous when sending sensitive info like SSH keys.

And I want to try Rust anyway, so here it is.

## Behavior

### Now

For now, actix-drop just writes received clipboard text to a file named after
the first 4 characters of its hexadecimal SHA256 hash.

For now, the files stay forever, and there's
no user separation/authentication. For secure usage, host it behind a VPN or
uses proxy like NGINX to request HTTP Basic Authentication.

### Planned (not yet implemented)

After the clipboard is received, a timer will be set for the file, and it will be
remove after some time.

I plan to write some basic configuration for actix-drop, e.g. the storage directory,
the timeout for file deletion

## Running actix-drop

By default, actix-drop listens on `localhost` port 3000.
It will use `./drop` as its storage.
