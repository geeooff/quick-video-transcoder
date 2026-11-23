# Quick Video Transcoder — French messages (fr). Regional files (fr-CA…) hold only what differs.
# Missing messages fall back to English. See docs/localization.md for the glossary and typography.

## Language menu (src/app/mod.rs)

menu-language = Langue
language-automatic = Automatique (langue du système)

## Timeline and transport (src/app/timeline.rs)

timeline-first-tooltip = Début de la partie conservée (Début)
timeline-previous-tooltip = Image précédente (Gauche)
timeline-play-tooltip = Lire / mettre en pause la partie conservée (Espace)
timeline-next-tooltip = Image suivante (Droite)
timeline-last-tooltip = Fin de la partie conservée (Fin)
timeline-start-here = [ Début ici
timeline-start-here-tooltip = Placer le début de la partie conservée sur l’image actuelle (I)
timeline-end-here = Fin ici ]
timeline-end-here-tooltip = Placer la fin de la partie conservée après l’image actuelle (O)
timeline-volume-tooltip = Volume : { NUM($percent) } %
timeline-mute-tooltip = Couper / rétablir le son (M)
timeline-no-sound = Cette vidéo n’a pas de son

## Settings panel (src/app/settings.rs)

settings-heading-selected =
    { $count ->
        [one] { NUM($count) } vidéo sélectionnée
       *[other] { NUM($count) } vidéos sélectionnées
    }
settings-change-applies-to-all = Toute modification s’applique à chacune
settings-no-video = Aucune vidéo sélectionnée

settings-section-profile = Profil
settings-section-kept-part = Partie conservée
settings-section-estimate = Estimation
settings-section-output = Sortie

settings-profile-changed = { $name } (modifié)
settings-profile-reset = Rétablir
settings-profile-reset-tooltip = Revenir aux réglages du profil
settings-profile-name-hint = Nom du profil
settings-profile-save = Enregistrer le profil
settings-profile-update = Mettre à jour le profil
settings-profile-save-tooltip = Garder ces réglages dans un profil pour les prochaines vidéos

settings-size-limit = Taille maximale
settings-size-limit-tooltip = En mégaoctets de 1 000 000 d’octets
settings-size-value = { NUM($mb, decimals: 1) } Mo
settings-resolution = Résolution
settings-resolution-up-to = jusqu’à { $short }p
settings-frame-rate = Fréquence d’images
settings-frame-rate-up-to = jusqu’à { NUM($fps) } i/s
settings-fps-value = { NUM($fps) } i/s
settings-codec = Codec
settings-codec-only = { $codec } uniquement
settings-codec-no-encoder = aucun encodeur
settings-codec-tooltip = Les codecs que la destination sait lire, par ordre de préférence : le premier que votre GPU sait encoder est utilisé, sinon le premier que le processeur sait encoder
settings-hdr = HDR
settings-hdr-convert = Convertir en SDR
settings-hdr-convert-tooltip = Décoché : une vidéo HDR reste en HDR (10 bits) ; les téléphones sans HDR l’affichent délavée
settings-hdr-convert-disabled-tooltip = H.264 ne prend pas en charge le HDR
settings-tone-mapping = Mappage tonal
settings-tone-mapping-disabled-tooltip = Cette vidéo est en SDR
settings-sound = Son
settings-sound-auto-tooltip = La piste du jeu : la première dont le nom n’indique ni un micro ni un chat vocal
settings-sound-mix-all = Toutes les pistes mixées
settings-sound-none = Sans son
settings-sound-auto-titled = Automatique : { $title }
settings-sound-auto-numbered = Automatique : piste { $number }
settings-sound-auto-default = Automatique (la piste du jeu)
settings-sound-track = Piste { $number }
settings-sound-bitrate = Débit audio
settings-kbps-value = { NUM($kbps) } kbit/s

settings-quality-bounds = Limites de qualité
settings-best-worth-it = Maximum utile
settings-best-worth-it-tooltip = Au-delà de cette qualité, les octets en plus ne se voient pas : un clip facile donne un fichier plus petit que la limite
settings-worst-accepted = Minimum accepté
settings-worst-accepted-tooltip = En dessous de cette qualité, la fréquence d’images, puis la résolution, sont réduites
settings-quality-value = QP { NUM($qp) } ({ $word })
settings-lowest-resolution = Résolution minimale
settings-lowest-frame-rate = Fréquence minimale
settings-file-name-suffix = Suffixe du nom

settings-kept-part-multi-note = La partie conservée se règle vidéo par vidéo, sur la chronologie
settings-trim-start = Début
settings-trim-end = Fin
settings-trim-duration = Durée
settings-keep-everything = Tout garder
settings-not-a-time = « { $text } » n’est pas un temps valide

settings-verdict-best-quality = Tient à la qualité maximale utile : le fichier sera plus petit que la limite
settings-verdict-fitted = Utilise toute la taille maximale
settings-verdict-below-floor = Sous la qualité minimale acceptée, même aux réglages les plus bas : raccourcissez la partie conservée
settings-estimate-output = Sortie
settings-estimate-output-value = { $size } à { $fps } i/s ({ $target })
settings-estimate-video-bitrate = Débit vidéo
settings-estimate-quality = Qualité
settings-estimate-quality-value = { $word } (QP { NUM($qp, decimals: 1) })
settings-estimate-size = Taille
settings-estimate-size-value = ≈ { NUM($predicted, decimals: 1) } Mo sur { NUM($limit, decimals: 1) } Mo
settings-estimate-measured = Mesurée sur
settings-estimate-measured-value =
    { $count ->
        [one] { NUM($count) } encodage d’essai
       *[other] { NUM($count) } encodages d’essai
    }
settings-estimating = Estimation…
settings-estimate-budget = Budget : { NUM($kbps) } kbit/s pour la vidéo
settings-estimate-filters = Filtres : { $engine }

settings-output-next-to-video = À côté de chaque vidéo
settings-output-use-next-to-video = À côté de chaque vidéo
settings-output-choose-folder = Choisir un dossier…
settings-output-folder-dialog-title = Dossier de sortie
settings-encoder = Encodeur
settings-encoder-automatic = Automatique
settings-parallel-encodes = Encodages simultanés
settings-parallel-measured-tooltip = Mesuré sur ce GPU : { $count } à la fois avant que les encodages ne se ralentissent entre eux (une RTX 4090 a deux moteurs NVENC, une RTX 3080 un seul)
settings-parallel-not-measured-tooltip = Pas encore mesuré
settings-parallel-measuring = mesure…
settings-parallel-measured = mesuré : { $count }
settings-parallel-not-measured = non mesuré
settings-parallel-use-measured = Appliquer
settings-parallel-measure-again = Mesurer à nouveau
settings-parallel-measure-again-tooltip = Quelques secondes, pendant lesquelles les nouvelles estimations et les encodages patientent

profiles-title = Profils
profiles-intro = Les nouvelles vidéos reçoivent le premier profil. Pour modifier un profil, enregistrez sous son nom les réglages d’une vidéo.
profiles-summary = { NUM($mb) } Mo · { $short }p{ NUM($fps) } · { $codecs }
profiles-summary-sdr = { NUM($mb) } Mo · { $short }p{ NUM($fps) } · { $codecs } · SDR
profiles-move-up-tooltip = Monter
profiles-delete-tooltip = Supprimer
profiles-add-built-in = Ajouter un profil intégré

## Preview (src/app/preview.rs)

preview-info = Aperçu { $size } · source { $width }x{ $height } { $codec }
preview-info-hdr-encoded = Aperçu { $size } · source { $width }x{ $height } { $codec } · { $dynamic } converti en SDR, comme à l’encodage
preview-info-hdr-kept = Aperçu { $size } · source { $width }x{ $height } { $codec } · { $dynamic } conservé, affiché en SDR
preview-seeking = Recherche…
preview-decoder-error = Décodeur : { $detail }
preview-empty-queue = Déposez vos captures de jeu ici, ou ajoutez-les avec Ctrl+O
preview-decoding = Décodage…
preview-select-video = Sélectionnez une vidéo

## Queue panel (src/app/queue_panel.rs)

queue-panel-heading = File d’attente
queue-panel-add = Ajouter…
queue-panel-add-tooltip = Ajouter des vidéos (Ctrl+O), ou les déposer ici
queue-panel-empty-title = Déposez des vidéos ici
queue-panel-empty-hint = ou passez par le menu Envoyer vers de l’Explorateur
queue-panel-row-details = { $summary } · { $duration } · { $profile }
queue-panel-duration-trimmed = { $kept } sur { $total }
queue-panel-reading = Analyse…
queue-panel-estimating = Estimation de la taille…
queue-panel-waiting = En attente d’un encodeur…
queue-panel-cancel = Annuler
queue-panel-progress =
    { NUM($percent) } %{ $speed ->
        [0] { "" }
       *[other] { " · " }{ NUM($speed, decimals: 1) }x
    }{ $retry ->
        [0] { "" }
       *[other] { " · " }reprise { NUM($retry) }
    }
queue-panel-done-size = { NUM($mb, decimals: 2) } Mo
queue-panel-copy = Copier
queue-panel-copy-tooltip = Copier le fichier pour le coller dans la conversation
queue-panel-show = Afficher
queue-panel-show-tooltip = Afficher dans l’Explorateur
queue-panel-estimate =
    { $target } · ≈ { NUM($mb, decimals: 1) } Mo · { $quality }{ $queued ->
        [yes] { " · " }en attente
       *[other] { "" }
    }
queue-panel-settings-changed = Réglages modifiés…
queue-panel-encode-selected = Encoder la sélection ({ NUM($count) })
queue-panel-encode-selected-tooltip = Encoder les vidéos sélectionnées selon leur estimation (Ctrl+E)
queue-panel-encode-all = Tout encoder ({ NUM($count) })
queue-panel-stop-all = Tout arrêter
queue-panel-copy-encoded = Copier les vidéos encodées ({ NUM($count) })
queue-panel-copy-encoded-tooltip = Copier tous les fichiers encodés pour les coller dans une conversation

## Main window: menus, status bar, log, shortcuts, dialogs (src/app/mod.rs)

menu-file = Fichier
menu-file-add-videos = Ajouter des vidéos…  (Ctrl+O)
menu-file-remove-selected = Retirer la sélection  (Suppr)
menu-file-remove-encoded = Retirer les vidéos encodées
menu-file-quit = Quitter
menu-profiles = Profils
menu-profiles-edit = Modifier les profils…
menu-profiles-open-folder = Ouvrir le dossier de profiles.toml
menu-view = Affichage
menu-view-log = Journal
menu-help = Aide
menu-help-shortcuts = Raccourcis clavier
menu-help-documentation = Documentation
menu-help-about = À propos de Quick Video Transcoder

status-summary =
    { $total ->
        [one] { NUM($total) } vidéo
       *[other] { NUM($total) } vidéos
    } · { NUM($encoding) } en encodage · { NUM($estimating) } en estimation · { NUM($done) } { $done ->
        [one] terminée
       *[other] terminées
    }
status-detecting = Détection des encodeurs…

shortcuts-title = Raccourcis clavier
shortcuts-play-keys = Espace
shortcuts-play-action = Lire / mettre en pause la partie conservée
shortcuts-step-keys = Gauche / Droite
shortcuts-step-action = Image précédente / suivante
shortcuts-second-keys = Maj + Gauche / Droite
shortcuts-second-action = −1 s / +1 s
shortcuts-ten-seconds-keys = Pg. préc. / Pg. suiv.
shortcuts-ten-seconds-action = −10 s / +10 s
shortcuts-trim-keys = I / O
shortcuts-trim-action = Début / fin de la partie conservée à l’image actuelle
shortcuts-goto-keys = Début / Fin
shortcuts-goto-action = Aller au début / à la fin de la partie conservée
shortcuts-mute-keys = M
shortcuts-mute-action = Couper / rétablir le son
shortcuts-add-keys = Ctrl + O
shortcuts-add-action = Ajouter des vidéos
shortcuts-select-all-keys = Ctrl + A
shortcuts-select-all-action = Sélectionner toutes les vidéos
shortcuts-encode-keys = Ctrl + E
shortcuts-encode-action = Encoder les vidéos sélectionnées
shortcuts-delete-keys = Suppr
shortcuts-delete-action = Retirer les vidéos sélectionnées
shortcuts-handle-keys = Faire glisser une poignée
shortcuts-handle-action = Déplacer le début ou la fin de la partie conservée
shortcuts-shift-drag-keys = Maj + glisser
shortcuts-shift-drag-action = Déplacer la partie conservée d’un bloc

title-idle =
    Quick Video Transcoder — { $total ->
        [one] { NUM($total) } vidéo
       *[other] { NUM($total) } vidéos
    }
title-encoding =
    Quick Video Transcoder — { $count ->
        [one] { NUM($count) } encodage en cours
       *[other] { NUM($count) } encodages en cours
    } ({ NUM($percent) } %)

dialog-add-title = Ajouter des vidéos
dialog-add-filter-videos = Vidéos
dialog-add-filter-all = Tous les fichiers
dialog-close-title = Encodages en cours
dialog-close-description = Fermer arrête les encodages en cours et supprime leurs fichiers partiels. Fermer quand même ?

app-profiles-load-failed = { $detail } ; utilisation du profil intégré
app-folder-not-video = { $path } est un dossier, pas une vidéo
app-files-copied =
    { $count ->
        [one] { NUM($count) } fichier copié : collez-le dans la conversation
       *[other] { NUM($count) } fichiers copiés : collez-les dans la conversation
    }
app-profiles-no-folder = aucun dossier de données : impossible d’enregistrer les profils
app-profiles-saved = profils enregistrés dans { $path }
app-ffmpeg-missing = { $program } est introuvable dans le PATH
app-ffmpeg-too-old = { $version } : il faut FFmpeg { NUM($required) } ou une version plus récente
app-hardware = matériel : { $summary }
app-no-encoder = FFmpeg fonctionne, mais aucun de ses encodeurs vidéo n’est utilisable sur cet ordinateur
app-measuring = mesure du nombre d’encodages simultanés possibles avec { $encoder }…
app-measure-failed = l’encodeur a échoué pendant la mesure des encodages simultanés
app-measured =
    { $key } : { $count ->
        [one] { NUM($count) } encodage simultané
       *[other] { NUM($count) } encodages simultanés
    }

## Help windows: About, FFmpeg, Send to (src/app/help.rs)

app-files-received =
    { $count ->
        [one] { NUM($count) } fichier reçu
       *[other] { NUM($count) } fichiers reçus
    } via Envoyer vers

sendto-menu-add = Ajouter au menu Envoyer vers de l’Explorateur
sendto-menu-update = Mettre à jour le raccourci Envoyer vers
sendto-menu-tooltip = Dans l’Explorateur, sélectionnez des vidéos, faites un clic droit, puis Envoyer vers > Quick Video Transcoder
sendto-menu-remove = Retirer du menu Envoyer vers de l’Explorateur
sendto-menu-written = Raccourci Envoyer vers créé : { $path }
sendto-menu-removed = Raccourci Envoyer vers supprimé

about-title = À propos de Quick Video Transcoder
about-heading = Quick Video Transcoder { $version }
about-summary = Découpe vos captures de jeu et les réencode pour tenir dans une taille maximale.
about-commit = Commit
about-documentation = Documentation
about-documentation-link = de cette version
about-changes = Historique
about-licence = Licence
about-built-with = Conçu avec
about-third-party-link = des bibliothèques tierces et leurs licences
about-notices-no-folder = aucun dossier de données où ouvrir les mentions des bibliothèques tierces
about-notices-failed = impossible d’écrire { $path } : { $detail }

ffmpeg-title = FFmpeg est nécessaire
ffmpeg-missing = Quick Video Transcoder lit et encode les vidéos avec FFmpeg, un programme gratuit qu’il n’inclut pas, et { $program } est introuvable sur cet ordinateur.
ffmpeg-too-old = La version de FFmpeg trouvée est trop ancienne ({ $version }). Il faut la version { NUM($required) } ou une version plus récente.
ffmpeg-installing = Installation de FFmpeg avec winget : une minute ou deux…
ffmpeg-winget-offer = Quick Video Transcoder peut l’installer pour vous avec winget, le gestionnaire de paquets de Windows : paquet { $package } (version complète de gyan.dev, environ 100 Mo, pour votre compte uniquement).
ffmpeg-install-button = Installer FFmpeg pour moi
ffmpeg-licence-note = L’installation vaut acceptation de la licence du paquet (GPL) et des conditions de la source winget.
ffmpeg-or-yourself = Ou installez-le vous-même :
ffmpeg-looking-for-winget = Recherche de winget…
ffmpeg-in-terminal = Dans le Terminal :
ffmpeg-copy = Copier
ffmpeg-download = Ou téléchargez la version « release full » sur
ffmpeg-unzip = décompressez-la, puis ajoutez son dossier bin à la variable d’environnement Path.
ffmpeg-check-again = Vérifier à nouveau
ffmpeg-install-log = installation de FFmpeg avec winget ({ $package })…

## Queue, estimate and encode messages (src/queue.rs, src/estimate.rs, src/encode.rs)

queue-probed =
    { $name } : { $summary }, { $duration }, { $tracks ->
        [one] { NUM($tracks) } piste audio
       *[other] { NUM($tracks) } pistes audio
    }, couleur { $color } ; { $engine }
queue-job-error = { $name } : { $detail }
queue-estimate-failed = { $name } : échec de l’estimation : { $detail }
queue-engine-changed = { $name } : passage à { $engine }
queue-encoded = { $name } : encodage terminé, { $path } ({ NUM($mb, decimals: 2) } Mo en { NUM($seconds, decimals: 1) } s)
queue-encode-failed = { $name } : échec de l’encodage : { $detail }
queue-no-estimate = impossible d’encoder sans estimation : { $detail }

estimate-quality-excellent = excellente
estimate-quality-very-good = très bonne
estimate-quality-good = bonne
estimate-quality-fair = correcte
estimate-quality-poor = médiocre
estimate-no-size = aucune taille de sortie à essayer
estimate-empty-range = la partie conservée est vide
estimate-too-small = { NUM($mb, decimals: 0) } Mo ne suffisent même pas pour le son de { $duration } : raccourcissez la partie conservée
estimate-sample-failed = échec de l’encodage d’essai ({ $status }) : { $detail }
estimate-sample-failed-silent = échec de l’encodage d’essai ({ $status }) : aucune sortie

encode-output-exists = { $path } existe déjà
encode-cannot-read = impossible de lire { $path }
encode-cannot-rename = impossible de renommer le fichier encodé en { $path }
encode-overshoot =
    l’encodeur a dépassé { NUM($mb, decimals: 1) } Mo { $count ->
        [one] { NUM($count) } fois
       *[other] { NUM($count) } fois
    } : baissez la qualité minimale ou raccourcissez la partie conservée
encode-ffmpeg-failed = échec de ffmpeg ({ $status }) : { $detail }
encode-ffmpeg-failed-silent = échec de ffmpeg ({ $status }) : aucun message
encode-refuse-partial = écriture de { $path } refusée : ce n’est pas un fichier partiel
encode-refuse-source = écriture refusée : { $path } est la source

## Profiles and settings values (src/profile.rs, src/plan.rs)

profile-tone-auto = Automatique (libplacebo)
profile-tone-bt2390 = BT.2390
profile-tone-spline = Spline
profile-tone-hable = Hable (filmique)
profile-tone-mobius = Möbius
profile-invalid-file = fichier de profils non valide
profile-cannot-read = impossible de lire { $path }
profile-cannot-create = impossible de créer { $path }
profile-cannot-write = impossible d’écrire { $path }
profile-cannot-replace = impossible de remplacer { $path }

plan-rate-whole = { NUM($fps, decimals: 0) }
plan-rate-fraction = { NUM($fps, decimals: 2) }

## Media details (src/ffmpeg/)

media-ffmpeg-start-failed = impossible de démarrer ffmpeg (FFmpeg est-il dans le PATH ?)
media-ffprobe-start-failed = impossible d’exécuter ffprobe (FFmpeg est-il dans le PATH ?)
media-ffprobe-failed = échec de ffprobe : { $detail }
media-probe-invalid = sortie de ffprobe non valide
media-no-video = aucun flux vidéo
media-no-dimensions = flux vidéo sans dimensions
media-no-frame = aucune image ({ $status }) : { $detail }

media-engine-vulkan = décodage Vulkan + libplacebo
media-engine-placebo = libplacebo
media-engine-software = filtres logiciels

media-dynamic-sdr = SDR
media-dynamic-pq = HDR10
media-dynamic-hlg = HLG

media-audio-layout =
    { $channels ->
        [1] mono
        [2] stéréo
        [6] 5.1
        [8] 7.1
       *[other] { NUM($channels) } canaux
    }
media-audio-track = { NUM($number) } · { $codec }, { $layout }
media-audio-track-titled = { NUM($number) } · { $title } ({ $codec }, { $layout })

media-summary = { $width }x{ $height } { $fps } i/s { $codec } { $dynamic }

media-hardware-summary =
    encodeurs : { $encoders } ; mappage tonal GPU (libplacebo) : { $placebo ->
        [yes] oui
       *[no] non
    }
media-hardware-none = aucun

media-winget-start-failed = impossible de démarrer winget
media-winget-failed = échec de winget ({ $status }) : { $detail }

## Startup, Send to, clipboard (src/main.rs, src/sendto.rs, src/clipboard.rs)

startup-window-failed-title = Quick Video Transcoder ne peut pas ouvrir sa fenêtre
startup-window-failed =
    { $detail }

    La fenêtre a besoin d’un pilote graphique compatible DirectX 12 ou Vulkan. Mettre à jour le pilote graphique règle généralement le problème.
startup-stopped-title = Quick Video Transcoder s’est arrêté
startup-stopped =
    Un problème est survenu : { $detail }

    Les encodages en cours n’ont pas abouti.
startup-log-hint = Les détails se trouvent dans { $path }

sendto-no-folder = aucun dossier Envoyer vers sur ce système
sendto-no-executable = impossible de trouver cet exécutable
sendto-no-exe-folder = l’exécutable n’a pas de dossier
sendto-start-failed = impossible de démarrer PowerShell
sendto-create-failed = impossible de créer { $path } : { $detail }
sendto-delete-failed = impossible de supprimer { $path }
sendto-shortcut-description = Découper et réencoder des vidéos pour tenir dans une taille maximale

clipboard-copy-failed = impossible de copier dans le Presse-papiers : { $detail }
clipboard-unsupported = la copie de fichiers dans le Presse-papiers n’est prise en charge que sous Windows
