# Video playback

Open a movie, episode, or home video and choose **Play** or **Resume**. Close the player to
save your current position. You can continue from the same account on another browser.

## Quality

Use **Quality** in the player to choose a maximum resolution and bitrate:

| Quality          | Maximum bitrate |
| ---------------- | --------------- |
| Original quality | No limit        |
| 1080p            | 8 Mbps          |
| 720p             | 4 Mbps          |
| 480p             | 1.5 Mbps        |

**Original quality** removes the resolution and bitrate limits. Koko may still convert
unsupported formats. Lower settings can help on slower connections, but conversion requires
server resources. Videos smaller than the selected resolution are not enlarged.

The player badge shows **Direct Play**, **Remuxing**, **Audio conversion**, or **Video conversion**,
so you can see how Koko is delivering the video. Original quality does not guarantee direct play.

If the browser rejects the original video, Koko tries compatible playback once automatically.
After successful recovery, quality changes keep compatible playback for that video until you
reload Koko.

Changing quality keeps your position, pause state, volume, and selected audio track. Each newly
opened video starts with **Original quality**. Use the audio track menu when a video has multiple tracks.
Use **Subtitles** to select an available sidecar subtitle track or turn subtitles off.

## Troubleshooting

- **Interrupted stream:** Check your connection, select a lower quality, or choose **Retry playback**.
- **Browser cannot decode the stream:** Choose **Try compatible playback** to request server conversion.
- **Server cannot start FFmpeg:** Ask your administrator to check the executable path and permissions
  in **Settings → Transcoding**. After correcting the settings, choose **Retry playback**.
- **Server cannot convert the media:** Try **Original quality**, or ask your administrator to check the
  source file and FFmpeg settings.
- **Media unavailable:** Choose another item. Ask your administrator to check library access and
  whether the file is still available, then rescan the library if needed.
- **Session expired:** Choose **Sign in again**, sign in, and reopen the video.

Retry and compatible playback continue from the current position when possible.
