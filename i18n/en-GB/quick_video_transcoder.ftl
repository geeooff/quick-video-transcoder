# British English: only the messages that differ from en (US English).

queue-probed =
    { $name }: { $summary }, { $duration }, { $tracks ->
        [one] { NUM($tracks) } audio track
       *[other] { NUM($tracks) } audio tracks
    }, colour { $color }; { $engine }

about-licence = Licence
about-third-party-link = third-party libraries and their licences
ffmpeg-licence-note = Installing accepts the package's licence (GPL) and the winget source's terms.
