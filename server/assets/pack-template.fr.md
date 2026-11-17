# Quidenous · modèle de pack de questions (pour une IA)

Ce fichier explique à une IA (ChatGPT, Claude…) comment écrire un **pack de questions** pour Quidenous, une app où un groupe d'amis répond chaque jour à une question : « Qui de nous… ? ». Lis tout, puis écris un fichier `.txt` qui respecte exactement ce format. Réponds avec le fichier seul, dans un bloc de code, sans commentaire autour.

## Format du fichier

Un fichier texte (UTF-8), **une question par ligne**. Les lignes vides sont ignorées.

### En-tête (en haut du fichier)

Des lignes `# clé: valeur`, avant la première question :

| Clé | Valeur | Obligatoire |
|---|---|---|
| `# titre:` | le nom du pack, 40 caractères max (court et accrocheur, idéalement moins de 32) | oui |
| `# description:` | une phrase, 140 caractères max (idéalement moins de 90) | conseillé |
| `# catégorie:` | 1 à 3 catégories parmi la liste ci-dessous, séparées par des virgules | conseillé |
| `# langue:` | `fr` ou `en` (la langue des questions) | oui |
| `# 16+:` | `oui` ou `non` | conseillé |

Toute autre ligne qui commence par `#` est un commentaire (ignoré).

### Catégories

- `potes` : la vie du groupe d'amis (habitudes, souvenirs, coloc, vacances).
- `pimente` : drague, amour, couple, questions osées. **Toujours 16+.** Rien d'explicite.
- `boulot` : travail, open space, études, collègues.
- `famille` : la famille, les repas, les vacances en famille, l'enfance.
- `soiree` : soirées, sorties, fêtes, lendemains difficiles.
- `culture` : films, musique, voyages, goûts, culture générale.
- `absurde` : hypothèses folles, situations improbables, scénarios absurdes.

Règle 16+ : `# 16+: oui` si le pack contient des questions osées. Un pack de catégorie `pimente` est toujours 16+ (même si la ligne dit `non`). Les questions osées n'ont leur place que dans un pack `pimente`.

### Les trois types de questions

1. **Qui de nous** (par défaut) : une ligne simple. Chacun vote pour une personne du groupe.
   `Qui oublierait son passeport le jour du départ ?`
2. **Classement** : la ligne commence par `[classement]`. Chacun classe les membres du groupe.
   `[classement] Classe-nous du plus ponctuel au plus en retard.`
3. **Qui choisirait quoi** : la ligne commence par `[choix]`, puis la question, puis 2 à 4 réponses séparées par `|`. Chacun choisit sa réponse et devine celles des autres.
   `[choix] Ton dîner idéal ? | Pizza | Sushi | Raclette`

Option : `[anonyme]` devant une question « qui de nous » ou un classement rend ses votes anonymes (seuls les totaux sont montrés), pour les questions où personne n'oserait voter à visage découvert. Il se combine : `[classement] [anonyme] …`. À garder pour quelques questions.

## Limites (sinon la ligne ou le pack est refusé)

- Question : 8 à 200 caractères (vise moins de 100).
- `[choix]` : 2 à 4 réponses, chacune de 1 à 40 caractères, toutes différentes.
- Pack : 5 à 300 questions (un bon pack en a 12 à 35). Pas de doublon.
- Titre 40 caractères max, description 140 max, 3 catégories max.

## Ton

- Court, drôle, bienveillant. Une question se lit en une seconde.
- Tutoiement. Les questions parlent du groupe : « Qui de nous… », « Qui… », « Classe-nous… ».
- Jamais de vrais noms, de marques de personnes réelles ou de célébrités visées.
- Rien de blessant : pas de moquerie sur le physique, le poids, la santé, l'origine, la religion, l'orientation ou l'argent de quelqu'un.
- Le piquant reste dans les packs `pimente`, sans contenu sexuel explicite.
- Mélange : environ deux tiers de « qui de nous », un quart de `[classement]`, quelques `[choix]`.

## Exemple 1

```text
# titre: Chroniques de coloc
# description: Vaisselle qui trempe, frigo partagé, post-it passifs-agressifs.
# catégorie: potes
# langue: fr
# 16+: non
Qui laisse une poêle « tremper » pendant quatre jours ?
Qui colle des post-it passifs-agressifs sur le frigo ?
Qui mange ton fromage en jurant que ce n'est pas lui ?
Qui ne descend jamais les poubelles mais remarque quand elles débordent ?
[classement] Classe-nous du plus bruyant au plus silencieux à travers une cloison.
[choix] Ta place idéale dans la coloc ? | La grande chambre | Près de la cuisine | Le canapé
```

## Exemple 2

```text
# titre: Avis brûlants sur l'amour
# description: Premiers rendez-vous, textos et grandes déclarations.
# catégorie: pimente, soiree
# langue: fr
# 16+: oui
Qui tomberait amoureux en une seule soirée ?
Qui relit dix fois un message avant de l'envoyer à son crush ?
Qui a déjà dragué avec une technique franchement douteuse ?
[classement] Classe-nous du plus romantique au plus cash.
[classement] Classe-nous de celui qui répond en 2 secondes à celui qui laisse en vu.
[choix] Le premier rendez-vous parfait ? | Un verre | Un ciné | Une balade | Un resto
```

## Prompt prêt à copier

> Lis ce modèle et écris un pack Quidenous de 20 questions sur [THÈME] pour un groupe d'amis, en français. Respecte exactement le format : en-tête complet (titre, description, catégorie, langue, 16+), environ 13 « qui de nous », 5 `[classement]` et 2 `[choix]` avec 2 à 4 réponses courtes. Réponds uniquement avec le contenu du fichier `.txt` dans un bloc de code.

Le fichier s'importe dans l'app : Packs → Publier un pack → Fichier .txt (l'en-tête pré-remplit le titre, la description, les catégories, la langue et le 16+).
