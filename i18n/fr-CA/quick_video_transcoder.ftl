# French (Canada): only the messages that differ from fr.

# Keyboard keys as Canadian French keyboards and Windows name them.

timeline-first-tooltip = Début de la partie conservée (Origine)
shortcuts-ten-seconds-keys = Pg préc / Pg suiv
shortcuts-goto-keys = Origine / Fin

# Québec typography: no space before ; ! ? (a no-break space stays before :).

settings-hdr-convert-tooltip = Décoché : une vidéo HDR reste en HDR (10 bits); les téléphones sans HDR l’affichent délavée
dialog-close-description = Fermer arrête les encodages en cours et supprime leurs fichiers partiels. Fermer quand même?
app-profiles-load-failed = { $detail }; utilisation du profil intégré
queue-probed =
    { $name } : { $summary }, { $duration }, { $tracks ->
        [one] { NUM($tracks) } piste audio
       *[other] { NUM($tracks) } pistes audio
    }, couleur { $color }; { $engine }
media-ffmpeg-start-failed = impossible de démarrer ffmpeg (FFmpeg est-il dans le PATH?)
media-ffprobe-start-failed = impossible d’exécuter ffprobe (FFmpeg est-il dans le PATH?)
media-hardware-summary =
    encodeurs : { $encoders }; mappage tonal GPU (libplacebo) : { $placebo ->
        [yes] oui
       *[no] non
    }
