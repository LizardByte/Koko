# Podcasts

Open **Podcasts** in the navigation to see your library. Select **Discover** to browse popular
shows or search by name. Choose a
two-letter country code, such as `US`, `CA`, or `GB`, to change the directory's region.
Popular shows come from Apple Podcasts. Browsing does not add any shows to your collection.
No API key or Apple account is required.

Select **Add** beside a show to subscribe. Your podcasts are available only to your Koko account.
Other users can add the same show to their own collections. **Remove** unsubscribes only your
account; it does not remove another user's access. Adding the show again restores your listening
history.

## Add a feed manually

Select **Add RSS feed**, enter the show's public RSS or Atom feed URL and select **Add podcast**.
Use a feed URL provided by the publisher, rather than a link to a podcast website or an Apple
Podcasts or Spotify page. Manual entry works even when the directory is unavailable.

Koko accepts HTTP and HTTPS feeds on the public internet. Feeds that require login credentials,
private network addresses, or a platform-specific subscription cannot be added.

## Browse and listen

Open a podcast to see its description, publisher, artwork, categories, and episodes. Shows that
publish season numbers use those seasons. Episodes without a season number are grouped by their
publication year. Episodes without a publication date appear under **Unknown year**.

The **Playback order** control on a podcast, season, or episode page chooses **Oldest to newest**
(the default) or **Newest to oldest**. Each podcast has its own setting, saved separately for your
account. Audio playback continues through the podcast in that order, across seasons and years.

Select **Play** beside an episode, or **Resume** to continue where you stopped. Audio episodes use
the persistent player, so you can keep browsing while listening. The player offers seeking,
volume, and playback speed. Video episodes use the video player. Listening progress and played
status are saved separately for each account.

Episode pages show publication dates, duration, explicit-content labels, participant names,
publisher links, transcripts, and chapter-document links when supplied by the publisher.
The amount of information varies between feeds.

Koko checks subscribed feeds for updates about once an hour while the server is running.
Select **Check for new episodes** to check a show immediately. Older episodes and listening
history remain listed when a publisher limits the number of episodes in its feed.

## Episode downloads

Koko saves episodes on the server and plays the saved copy on subsequent listens. New episodes
found after you add a podcast download automatically. Older episodes download when you first play
them. The first play may take longer while the download finishes. Episode details show whether a
copy is queued, downloading, downloaded, or failed.

Administrators can set **Podcast download directory** under **Settings → General → Storage**.
Leave it empty to use the `podcasts` folder inside Koko's data directory. The server needs write
access and sufficient free space. Changing the directory affects new downloads; existing saved
copies remain available in their current location.

Turn off **Download new podcast episodes automatically** to download episodes only when played.
Downloaded copies are shared when several users subscribe to the same show. Removing a subscription
does not delete saved episodes. Saved episodes can play when the publisher is unavailable, but your
device still needs a connection to your Koko server.

## Troubleshooting

- **A show is missing from search:** Add its RSS feed URL manually. Some shows do not provide a public feed.
- **An episode does not play:** Check that the publisher still offers the episode. The initial download requires an internet connection and enough writable disk space on the server. Check **Media details** for a download error. Playback requires a browser-supported audio or video format.
- **The directory is unavailable:** Try again later or add the publisher's feed URL directly. Existing subscriptions remain available.
- **The feed cannot be added:** Verify that the URL returns a podcast feed and is reachable from the Koko server. A regular website, a private address, or a feed requiring login credentials will not work.
- **Updates have stopped:** Select **Check for new episodes** and check the show's page for a feed-check error. A publisher outage does not erase your listening history.
