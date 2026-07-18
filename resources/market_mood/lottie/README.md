# Origin

The moods here were created from the following


# Telegram vs WhatsApp approach

The provided media are vectorial animations in the lottie. Telegram accepts them, but WhatsApp don't.
Since the lottie .tgs files are much, much smaller, we'll be storing them and deriving the WhatsApp versions
at build time.

There is a script provided for that called `./tgs_to_whatsapp.sh`

TODO: create a workitem for this build system
TODO: edit the .tgs files and add the bot name & url there.