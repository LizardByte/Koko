- [x] web-ui: player does not resume for direct play items, it starts over from the beginning (though the progress bar looks like it's resuming from the old position)... it is possible actually to resume? probably start playback and auto seek to the position?
- [x] web-ui: home-preview if highlighting a season or episode in recently added row, it should show the info from the show in the preview since seasons and episodes do not have this info
- [x] web-ui: when viewing a person, don't show seasons and episodes they are in by default, just show the top level show... then if the show is highlighted show the seasons in like an expanding tray... and then if a season is highlighted, show the episodes they are in another tray
- [x] web-ui: when viewing a collection it should open a dedicated page like a tv show page or season page instead of what it's doing now which is just like a filtered library view
- [x] web-ui: make playlists and genres load a page, just like collections now
- [x] web-ui: on the collection tab (and probably playlist/genre) the hero should be based on the first item in the collection, and then update when highlighting a different item
- [x] web-ui: on the collection page, it should look just like the library page and show the collection poster for the card with the collection title below
- [x] web-ui: when viewing an item, show other items which are in the same collection as this item, if the current item is in a collection... if it's in multiple collections, have rails for each one... if this is the only item in the collection, then don't show a rail for it
- [x] web-ui: refreshing metadata shows spinner on all items, even if they are already finished
- [x] web-ui: make background for spinner smaller so it is not ~2x bigger than the spinner, but only slightly bigger... if needed make spinner bigger to fix
- [x] web-ui: general issue where when data comes in the page almost reloads or rebuilds the dom and then resets the position where the user has scrolled to for example... especially noticeable on library views with rails like recommended and recently added

- [x] add a "scanner" module, which is responsible for file scanning, we should have different types of scanners
  - [x] movies scanner
  - [x] tv shows scanner
  - [o] music scanner
  - [o] photos scanner
  - [o] books scanner
- [x] store media file hash in database to know when a file has changed, when it changes we should re-run ffprobe stuff
- [x] the scanner should be responsible for deciding what to store as the hash value
- [x] have a base scanner (directory) module with the other modules as their own rs files... hashing can be common from the  base scanner, but future scanner may use different hashing method
- [x] each library should have a scanner option, and default to the obvious scanner for each library type... this should have no affect on the metadata provider other than what the scanner puts into the db for the initial searching (I think)

- [x] scheduled-tasks: add a scheduled tasks module, which is responsible for running scheduled tasks, we should have different types of tasks
  - [x] have option to set when scheduled tasks start and stop, e.g. only run between 2am and 6am, could also set the days of the week
  - [x] metadata refresh task, which will refresh metadata for items in the library on a regular basis
  - [x] clean/vaccum task, which will clean up the database and vacuum it on a regular basis (optional, but on by default)
- [ ] scheduled-tasks: add task to get thumbnail images from video files, to be used for seeking in the UI (this needs to be optional, and off by default because the pictures will take up a lot of space... the images should be smaller, and not the full resolution of the frame)

- [ ] database: consolidate all migrations into one migration, since we've never had a release yet and no users... this will probably allow removing quite a bit of stuff which was only done temporarily and then reverted?
- [ ] database: remove hacks in db.rs which were mostly used to repair migrations that already ran and then were modified after the fact, thus the updated migrations never ran
- [ ] database: clearly document how the migrations work, and ensure we do not add these kind of hacks again in the future... using the migration system should be the only way to modify the database schema
- [x] database: media files should be a one-to-many relationship as they can be part of multiple libraries
- [x] database: add a table for external media, such as youtube video urls (but also from other online sources)... the theme songs/trailers would point to the external media table instead of storing the url directly in the metadata tables (allow a lot of deduplication)
- [x] database: add a type for extras (support all the types from trailerdb.org at a minimum as well as a theme song type)
- [x] database: keep track of watched items, how many times they were watched, and when they were watched last... for each user... if watched we should show a watched icon on the item and maybe a (circular?) progress bar for partially watched items

- [ ] permissions: properly guard api endpoints... many need to be admin only, while others would be allowed for standard users... users who have not been granted access to a library should not be able to see any information about that library or the items in it from the api or ui
- [ ] permissions: ensure non admins do not have admin functions in the UI (no scanning, no metadata refresh, no user management, etc...)

- [ ] extras: add external players for extras
  - [ ] yt-dlp (https://github.com/yt-dlp/yt-dlp)
  - [ ] streamlink (https://streamlink.github.io)
  - [ ] vlc (https://www.videolan.org/vlc/)
  - [ ] mpv (https://mpv.io/)

- [x] metadata: prefer svg images for icons/logos, if svg not available png is okay
- [x] metadata: TVDB does not automatch very well, I think it's something with default values for the search not being correctly applied in all cases
- [x] metadata: for tv shows, show missing seasons and missing episodes in the ui with a "missing" badge or something... will require collecting metadata for all seasons and episodes of a show, not just the ones that are in the library
- [x] metadata: tmdb - add guest stars (people) for episodes
- [x] metadata: tmdb/tvdb - can metadata refresh speed be optimized, especially for tv shows? very slow right now
  - maybe people collection can be done afterwards? this might avoid a lot of duplication?
- [x] web-ui/metadata: when showing X seasons badge, it should show the number of seasons we have episodes in, and not count missing seasons

- [x] metadata-providers: decouple from metadata/mod.rs as much as possible
- [x] metadata-providers: add a template to make adding providers easier, make sure it's very well documented
- [x] metadata-providers: need to have primary and secondary providers
- [x] metadata-providers: move ThemerrDB to a secondary provider that extends tmdb and tvdb
- [x] metadata-providers: decouple Themerr from main code and only have it in its own provider module. We should drop the theme url into our database, just like all other metadata... we may need to add some YouTube url helpers as it seems many providers are using YouTube videos directly
- [x] metadata-providers: add collection support for ThemerrDB (https://github.com/LizardByte/ThemerrDB/tree/database/movie_collections)... may require adding a new entry for supported_kinds
- [x] metadata-providers: add overview for collections and other data available from tmdb
- [x] metadata-providers/database: collections have duplicate entries in the database when multiple movies are in the same collection, need to de-duplicate this and have a single entry for the collection with a many-to-many relationship to items
- [x] metadata-providers/database: themerr should not set "name" column for collections
- [x] metadata-providers: allow getting trailers from tvdb
- [x] metadata-providers: tvdb does not get biography for people
- [x] metadata-providers: add trailerdb (https://trailerdb.org/api-docs) as a secondary provider for movies and shows, it should be able to extend tmdb
- [x] metadata-providers: tmdb and tvdb... store external ids such as imdb id, and all external ids for people
- [x] metadata-providers: after tvdb has external ids, themerr can use the imdb id to get theme songs for tvdb

- [x] settings: settings.yml file should be extremely minimal, with everything else living in the database
- [x] settings: can tmdb and tvdb api_keys (and other keys/tokens/passwords) be encrypted somehow?
- [x] settings: language for providers should not be a global setting, but should be a per library option...
- [x] settings: provider settings should be their own modal/page instead of in the main settings... each provider with their own settings
- [x] settings: creating libraries needs to be revamped
  - [x] we need to have permissions for libraries, e.g. who is allowed to view them
  - [x] put providers in a vertical list
  - [x] allow re-ordering the provider preference
  - [x] add button to provider settings
  - [x] have dropdown for language, and allow multiple selections

- [x] users: add ability for profile image upload instead of providing a url to an online hosted image

- [x] ui/ux: Collection pages should be almost identical to a TV Show page, same for playlists and categories
- [x] ui/ux: manual linkage search has one provider selected, but it's not necessarily the library default options... should be the default providers of the current library
- [x] ui/ux: when searching live, if you type "top " (with a trailing space) and wait a second, it will remove the space, and then you cannot continue typing without adding another space... the search could do a strip, but we should leave the search text field as is
- [x] ui/ux: when searching, live or full, we should make the home preview show the highlighted search result
- [x] ui/ux: after doing a full search there is no way to return to the main library
- [x] ui/ux: live search box should have an "x" to close it out and reset the search field
- [x] ui/ux: Manual linkage search may return results in movie's native language instead of library language... it should return in library language (T-34 with TVDB)
- [x] ui/ux: Manual linkage search selected providers should only default to the current library enabled provider(s)
- [x] ui/ux: Show attribution image in manual search results instead of name, can fall back to name if no image is available
- [x] ui/ux: TMDB and TVDB providers should have a search score penalty if the casing is off (uppercase vs lowercase)
- [x] ui/ux: home/library search should be cross library for both live and full results... no matter what library is currently selected
- [x] ui/ux: home/library search should search all item types (movies, shows, seasons, episodes, collections, playlists, people, and future types)
- [x] ui/ux: Allow for more than 12 items in home/library rows, but they should lazy load

- [x] home/library pages: for tv show libraries, recommended should not be individual seasons or episodes, just shows

- [x] item-pages: ThemerrDB themes are not working anymore... remember when navigating between a show, season, and episode within the same show the theme song should not restart
- [x] item-pages: Very slow to load episode thumbnails when viewing a season of a show. Probably need to revamp how metadata is stored... Fetching a lot of shows from the database at once is probably slow?
  - batched primary metadata-link loading for item and child lists to remove per-item metadata-link queries (N+1), which speeds season episode grids and thumbnail metadata resolution

- [ ] tests: Move all rust tests to tests directory instead of just directly in the source file. Need to carefully expose private code to tests. I believe there are ways to do this in rust cleanly.
- [ ] tests: Add tests for web ui client... maybe use playwright?

- [ ] features: add podcast support, users should be able to add the podcasts they are specifically interested in and it should appear like a library with shows and episodes... we could also allow users to add custom rss feeds for podcasts that are not in the providers... not 100% sure how to search for podcasts and get metadata... any free APIs for this? maybe use a provider like itunes search api or something? https://affiliate.itunes.apple.com/resources/documentation/itunes-store-web-service-search-api/ ... this would be a new provider module for podcasts, and we would need to add a new media type for podcast episodes... the player would also need to be revamped to support audio only playback with a static image instead of a video... we could also add support for showing the episode transcript if available, maybe even with a karaoke style highlighting of the current line as it is being spoken
  - ref: https://github.com/advplyr/audiobookshelf/blob/master/server/providers/iTunes.js
- [ ] features: add local music library support using musicbrainz as a metadata provider with crate https://crates.io/crates/musicbrainz_rs... this would be similar to podcasts but with albums and tracks instead of shows and episodes... the player would also need to be revamped to support audio only playback with a static image instead of a video (no more than 1 api call per second)
- [ ] features: can we link people from tmdb/tvdb to musicbrainz people? I think tmdb has external ids for the imdb id, and so do musicbrainz, so we could use that as a common key to link them together
  - actually each db has external ids, so we can store all the external ids to link tmdb people to tvdb people, or any other provider
  - tmdb external ids:
    - Facebook
    - IMDb
    - Instagram
    - TicTok
    - Twitter
    - Wikidata
    - YouTube
  - tvdb external ids (not sure, but I think it's under remoteIds of extended people results)
  - musicbrainz external ids (https://community.metabrainz.org/t/how-can-i-retrieve-the-list-of-external-links-annotation-and-wikipedia-text-via-api/621544/3):
    - Facebook
    - IMDb
    - Instagram
    - Twitter
    - Wikidata
    - YouTube
    - any many many more
- [ ] features: add parental controls, with content limits based on age and rating... time limits... and reports... don't only report on what was watched, but also on items that were viewed
- [ ] features: add ability to easily rate content after watching... simple thumbs up/down... after a positive rating can show similar/related content
- [ ] features: watch together and have animated emoji reactions (https://www.npmjs.com/package/@remotion/animated-emoji)
  - allow watching together with another person on a different client
  - if a person reacts with an emoji, show that emoji animating on all the clients (that are part of the watch together party) in real time

- [x] licensing: add LB license
- [ ] licensing: figure out how to monetize
  - https://polar.sh/
  - https://dodopayments.com/blogs/polar-alternatives-saas-billing
