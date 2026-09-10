import type { RecipientScope } from "$lib/types";

export const CRYPTO_METHODS = ["nip44_encrypt", "nip44_decrypt", "nip04_encrypt", "nip04_decrypt"] as const;
export type CryptoMethod = (typeof CRYPTO_METHODS)[number];
export type CryptoPermissions = Partial<Record<CryptoMethod, RecipientScope>>;
export type UseCaseItem = { name: string; kinds: number[]; default: boolean; crypto?: CryptoPermissions };
export type PolicyUseCase = {
    id: string; name: string; category: string; description: string; nips: string[];
    items: UseCaseItem[]; crypto?: CryptoPermissions;
    transport?: { name: string; kind: number; description: string }[];
    notes?: string[]; sources?: string[];
};

// Curated permission snapshots. Do not reinterpret saved documents using catalog defaults.
// See docs/development/policy-catalog.md for coverage, role boundaries, and exclusions.
export const NIPS_REVISION = "a2494f4f81d46684e5814a9bf35e2b1df978f955";
export const MARMOT_REVISION = "4a2bc65f8db5866cec3b2a127dedb37818eaf207";
export const POLICY_USE_CASES: readonly PolicyUseCase[] = [
    {
        "id": "notes",
        "name": "Notes and replies",
        "category": "Social",
        "nips": [
            "01",
            "10"
        ],
        "description": "Notes and replies",
        "items": [
            {
                "name": "Notes and replies",
                "kinds": [
                    1
                ],
                "default": true
            }
        ],
        "notes": []
    },
    {
        "id": "longform",
        "name": "Long-form publishing",
        "category": "Social",
        "nips": [
            "23",
            "37"
        ],
        "description": "Articles, encrypted drafts, and revision history",
        "items": [
            {
                "name": "Published articles",
                "kinds": [
                    30023
                ],
                "default": true
            },
            {
                "name": "Encrypted drafts",
                "kinds": [
                    31234
                ],
                "default": true,
                "crypto": {
                    "nip44_encrypt": "self_only",
                    "nip44_decrypt": "self_only"
                }
            },
            {
                "name": "Draft revision history",
                "kinds": [
                    1234
                ],
                "default": true,
                "crypto": {
                    "nip44_encrypt": "self_only",
                    "nip44_decrypt": "self_only"
                }
            },
            {
                "name": "Private relay preferences",
                "kinds": [
                    10013
                ],
                "default": false,
                "crypto": {
                    "nip44_encrypt": "self_only",
                    "nip44_decrypt": "self_only"
                }
            },
            {
                "name": "Legacy article drafts",
                "kinds": [
                    30024
                ],
                "default": false,
                "crypto": {
                    "nip04_encrypt": "self_only",
                    "nip04_decrypt": "self_only"
                }
            }
        ],
        "notes": [
            "Draft permissions also cover drafts of other content types."
        ]
    },
    {
        "id": "comments",
        "name": "Comments",
        "category": "Social",
        "nips": [
            "22"
        ],
        "description": "Comments on articles and other content",
        "items": [
            {
                "name": "Comments on articles and other content",
                "kinds": [
                    1111
                ],
                "default": true
            }
        ],
        "notes": []
    },
    {
        "id": "reactions",
        "name": "Reactions",
        "category": "Social",
        "nips": [
            "25"
        ],
        "description": "Nostr reactions, Reactions to websites",
        "items": [
            {
                "name": "Nostr reactions",
                "kinds": [
                    7
                ],
                "default": true
            },
            {
                "name": "Reactions to websites",
                "kinds": [
                    17
                ],
                "default": false
            }
        ],
        "notes": []
    },
    {
        "id": "reposts",
        "name": "Reposts",
        "category": "Social",
        "nips": [
            "18"
        ],
        "description": "Note reposts, Other content reposts",
        "items": [
            {
                "name": "Note reposts",
                "kinds": [
                    6
                ],
                "default": true
            },
            {
                "name": "Other content reposts",
                "kinds": [
                    16
                ],
                "default": true
            }
        ],
        "notes": []
    },
    {
        "id": "profile",
        "name": "Profile",
        "category": "Identity and preferences",
        "nips": [
            "01",
            "05",
            "24"
        ],
        "description": "Profile name, avatar, and bio",
        "items": [
            {
                "name": "Profile name, avatar, and bio",
                "kinds": [
                    0
                ],
                "default": true
            }
        ],
        "notes": []
    },
    {
        "id": "follows",
        "name": "Following and starter packs",
        "category": "Identity and preferences",
        "nips": [
            "02",
            "51"
        ],
        "description": "Follow list, Named follow sets, Starter packs, Media follows, Media starter packs",
        "items": [
            {
                "name": "Follow list",
                "kinds": [
                    3
                ],
                "default": true
            },
            {
                "name": "Named follow sets",
                "kinds": [
                    30000
                ],
                "default": true
            },
            {
                "name": "Starter packs",
                "kinds": [
                    39089
                ],
                "default": true
            },
            {
                "name": "Media follows",
                "kinds": [
                    10020
                ],
                "default": false
            },
            {
                "name": "Media starter packs",
                "kinds": [
                    39092
                ],
                "default": false
            }
        ],
        "notes": [
            "Private list entries need separate self-only encryption and decryption access."
        ]
    },
    {
        "id": "status",
        "name": "Profile status",
        "category": "Identity and preferences",
        "nips": [
            "38"
        ],
        "description": "Status updates",
        "items": [
            {
                "name": "Status updates",
                "kinds": [
                    30315
                ],
                "default": true
            }
        ],
        "notes": []
    },
    {
        "id": "relays",
        "name": "Relay preferences",
        "category": "Identity and preferences",
        "nips": [
            "65",
            "51"
        ],
        "description": "Read and write relays, Search relays, Blocked relays, Relay feeds, Named relay sets",
        "items": [
            {
                "name": "Read and write relays",
                "kinds": [
                    10002
                ],
                "default": true
            },
            {
                "name": "Search relays",
                "kinds": [
                    10007
                ],
                "default": true
            },
            {
                "name": "Blocked relays",
                "kinds": [
                    10006
                ],
                "default": true
            },
            {
                "name": "Relay feeds",
                "kinds": [
                    10012
                ],
                "default": true
            },
            {
                "name": "Named relay sets",
                "kinds": [
                    30002
                ],
                "default": true
            }
        ],
        "notes": [
            "Private list entries need separate self-only encryption and decryption access."
        ]
    },
    {
        "id": "bookmarks",
        "name": "Bookmarks and pins",
        "category": "Collections",
        "nips": [
            "51"
        ],
        "description": "Bookmarks, Bookmark folders, Pinned notes",
        "items": [
            {
                "name": "Bookmarks",
                "kinds": [
                    10003
                ],
                "default": true
            },
            {
                "name": "Bookmark folders",
                "kinds": [
                    30003
                ],
                "default": true
            },
            {
                "name": "Pinned notes",
                "kinds": [
                    10001
                ],
                "default": true
            }
        ],
        "notes": [
            "Private list entries need separate self-only encryption and decryption access."
        ]
    },
    {
        "id": "curation",
        "name": "Content collections",
        "category": "Collections",
        "nips": [
            "51"
        ],
        "description": "Article collections, Video collections, Picture collections",
        "items": [
            {
                "name": "Article collections",
                "kinds": [
                    30004
                ],
                "default": true
            },
            {
                "name": "Video collections",
                "kinds": [
                    30005
                ],
                "default": true
            },
            {
                "name": "Picture collections",
                "kinds": [
                    30006
                ],
                "default": true
            }
        ],
        "notes": [
            "Private list entries need separate self-only encryption and decryption access."
        ]
    },
    {
        "id": "interests",
        "name": "Topics and interests",
        "category": "Collections",
        "nips": [
            "51"
        ],
        "description": "Interests, Topic collections",
        "items": [
            {
                "name": "Interests",
                "kinds": [
                    10015
                ],
                "default": true
            },
            {
                "name": "Topic collections",
                "kinds": [
                    30015
                ],
                "default": true
            }
        ],
        "notes": [
            "Private list entries need separate self-only encryption and decryption access."
        ]
    },
    {
        "id": "emojis",
        "name": "Custom emoji collections",
        "category": "Collections",
        "nips": [
            "30",
            "51"
        ],
        "description": "Favorite emojis, Emoji sets",
        "items": [
            {
                "name": "Favorite emojis",
                "kinds": [
                    10030
                ],
                "default": true
            },
            {
                "name": "Emoji sets",
                "kinds": [
                    30030
                ],
                "default": true
            }
        ],
        "notes": [
            "Private list entries need separate self-only encryption and decryption access."
        ]
    },
    {
        "id": "mutes",
        "name": "Mute lists",
        "category": "Collections",
        "nips": [
            "51"
        ],
        "description": "Muted accounts and content, Mute sets by kind",
        "items": [
            {
                "name": "Muted accounts and content",
                "kinds": [
                    10000
                ],
                "default": true
            },
            {
                "name": "Mute sets by kind",
                "kinds": [
                    30007
                ],
                "default": true
            }
        ],
        "notes": [
            "Private list entries need separate self-only encryption and decryption access."
        ]
    },
    {
        "id": "dm",
        "name": "NIP-17 direct messages",
        "category": "Direct messaging",
        "nips": [
            "17",
            "59"
        ],
        "description": "Private text and file messages using NIP-59 gift wraps",
        "items": [
            {
                "name": "Message seals",
                "kinds": [
                    13
                ],
                "default": true,
                "crypto": {
                    "nip44_encrypt": "any",
                    "nip44_decrypt": "any"
                }
            },
            {
                "name": "Messaging relay preferences",
                "kinds": [
                    10050
                ],
                "default": true
            }
        ],
        "notes": [],
        "transport": [
            {
                "name": "Gift wraps",
                "kind": 1059,
                "description": "The app signs the outer wrapper with a fresh temporary key. Receiving it uses NIP-44 decryption; no identity signing permission for kind 1059 is needed."
            },
            {
                "name": "Message contents",
                "kind": 14,
                "description": "Text rumors (14), file rumors (15), and reactions (7) inside the seal are unsigned."
            }
        ]
    },
    {
        "id": "chat",
        "name": "Chat messages",
        "category": "Messaging",
        "nips": [
            "C7",
            "29"
        ],
        "description": "Chat messages",
        "items": [
            {
                "name": "Chat messages",
                "kinds": [
                    9
                ],
                "default": true
            }
        ],
        "notes": []
    },
    {
        "id": "forums",
        "name": "Forum threads",
        "category": "Messaging",
        "nips": [
            "7D",
            "29"
        ],
        "description": "Forum threads",
        "items": [
            {
                "name": "Forum threads",
                "kinds": [
                    11
                ],
                "default": true
            }
        ],
        "notes": []
    },
    {
        "id": "publicmsg",
        "name": "Public messages",
        "category": "Messaging",
        "nips": [
            "A4"
        ],
        "description": "Public messages",
        "items": [
            {
                "name": "Public messages",
                "kinds": [
                    24
                ],
                "default": true
            }
        ],
        "notes": []
    },
    {
        "id": "marmot",
        "name": "Marmot messaging",
        "category": "Direct messaging",
        "nips": [],
        "description": "MLS messaging, key packages, account proofs, and welcome gift wraps",
        "items": [
            {
                "name": "Publish key packages",
                "kinds": [
                    30443
                ],
                "default": true
            },
            {
                "name": "Authorize MLS account identity",
                "kinds": [
                    450
                ],
                "default": true
            },
            {
                "name": "Seal welcome invitations",
                "kinds": [
                    13
                ],
                "default": true,
                "crypto": {
                    "nip44_encrypt": "any"
                }
            },
            {
                "name": "Messaging inbox relays",
                "kinds": [
                    10050
                ],
                "default": true
            },
            {
                "name": "Publish read and write relays",
                "kinds": [
                    10002
                ],
                "default": false
            },
            {
                "name": "Push notification owner proofs",
                "kinds": [
                    451
                ],
                "default": false
            },
            {
                "name": "Legacy key packages",
                "kinds": [
                    443
                ],
                "default": false
            },
            {
                "name": "Legacy key package relays",
                "kinds": [
                    10051
                ],
                "default": false
            }
        ],
        "crypto": {
            "nip44_decrypt": "any"
        },
        "transport": [
            {
                "name": "Gift wraps",
                "kind": 1059,
                "description": "The app signs the outer wrapper with a fresh temporary key. Receiving it uses NIP-44 decryption; no identity signing permission for kind 1059 is needed."
            },
            {
                "name": "MLS welcome",
                "kind": 444,
                "description": "The welcome inside its seal is unsigned."
            },
            {
                "name": "MLS group messages",
                "kind": 445,
                "description": "Group transport uses temporary signing keys and MLS-derived encryption in the app."
            }
        ],
        "notes": [
            "MLS group encryption stays in the client. Keycast provides the account signatures and welcome decryption."
        ],
        "sources": [
            "https://github.com/marmot-protocol/marmot/blob/4a2bc65f8db5866cec3b2a127dedb37818eaf207/transports/nostr.md",
            "https://github.com/marmot-protocol/marmot/blob/4a2bc65f8db5866cec3b2a127dedb37818eaf207/app-components/account-identity-proof-v2.md"
        ]
    },
    {
        "id": "giftwrap",
        "name": "NIP-59 gift wrapping",
        "category": "Direct messaging",
        "nips": [
            "59"
        ],
        "description": "Seal and open gift-wrapped content (1059)",
        "items": [
            {
                "name": "Sign message seals",
                "kinds": [
                    13
                ],
                "default": true,
                "crypto": {
                    "nip44_encrypt": "any"
                }
            }
        ],
        "crypto": {
            "nip44_decrypt": "any"
        },
        "transport": [
            {
                "name": "Gift wraps",
                "kind": 1059,
                "description": "The app signs the outer wrapper with a fresh temporary key. Receiving it uses NIP-44 decryption; no identity signing permission for kind 1059 is needed."
            }
        ],
        "notes": []
    },
    {
        "id": "groups",
        "name": "Group membership",
        "category": "Messaging",
        "nips": [
            "29",
            "51"
        ],
        "description": "Join groups, Leave groups, Group list",
        "items": [
            {
                "name": "Join groups",
                "kinds": [
                    9021
                ],
                "default": true
            },
            {
                "name": "Leave groups",
                "kinds": [
                    9022
                ],
                "default": true
            },
            {
                "name": "Group list",
                "kinds": [
                    10009
                ],
                "default": true
            }
        ],
        "notes": [
            "Private list entries need separate self-only encryption and decryption access."
        ]
    },
    {
        "id": "pictures",
        "name": "Picture publishing",
        "category": "Media",
        "nips": [
            "68"
        ],
        "description": "Picture posts",
        "items": [
            {
                "name": "Picture posts",
                "kinds": [
                    20
                ],
                "default": true
            }
        ],
        "notes": []
    },
    {
        "id": "video",
        "name": "Video publishing",
        "category": "Media",
        "nips": [
            "71"
        ],
        "description": "Videos, Short videos, Addressable videos, Addressable short videos",
        "items": [
            {
                "name": "Videos",
                "kinds": [
                    21
                ],
                "default": true
            },
            {
                "name": "Short videos",
                "kinds": [
                    22
                ],
                "default": true
            },
            {
                "name": "Addressable videos",
                "kinds": [
                    34235
                ],
                "default": false
            },
            {
                "name": "Addressable short videos",
                "kinds": [
                    34236
                ],
                "default": false
            }
        ],
        "notes": []
    },
    {
        "id": "voice",
        "name": "Voice messages",
        "category": "Media",
        "nips": [
            "A0"
        ],
        "description": "Voice messages, Voice replies",
        "items": [
            {
                "name": "Voice messages",
                "kinds": [
                    1222
                ],
                "default": true
            },
            {
                "name": "Voice replies",
                "kinds": [
                    1244
                ],
                "default": true
            }
        ],
        "notes": []
    },
    {
        "id": "files",
        "name": "File metadata",
        "category": "Media",
        "nips": [
            "94"
        ],
        "description": "File metadata",
        "items": [
            {
                "name": "File metadata",
                "kinds": [
                    1063
                ],
                "default": true
            }
        ],
        "notes": []
    },
    {
        "id": "blossom",
        "name": "Media server preferences",
        "category": "Media",
        "nips": [
            "B7",
            "51"
        ],
        "description": "Choose file servers; upload authorization is separate",
        "items": [
            {
                "name": "Blossom servers",
                "kinds": [
                    10063
                ],
                "default": true
            }
        ],
        "notes": [
            "Private list entries need separate self-only encryption and decryption access."
        ]
    },
    {
        "id": "livehost",
        "name": "Host live events",
        "category": "Media",
        "nips": [
            "53"
        ],
        "description": "Live streams, Audio spaces, Meetings",
        "items": [
            {
                "name": "Live streams",
                "kinds": [
                    30311
                ],
                "default": true
            },
            {
                "name": "Audio spaces",
                "kinds": [
                    30312
                ],
                "default": true
            },
            {
                "name": "Meetings",
                "kinds": [
                    30313
                ],
                "default": true
            }
        ],
        "notes": []
    },
    {
        "id": "livechat",
        "name": "Live event participation",
        "category": "Media",
        "nips": [
            "53"
        ],
        "description": "Live chat, Room presence",
        "items": [
            {
                "name": "Live chat",
                "kinds": [
                    1311
                ],
                "default": true
            },
            {
                "name": "Room presence",
                "kinds": [
                    10312
                ],
                "default": true
            }
        ],
        "notes": []
    },
    {
        "id": "podcast",
        "name": "Podcast publishing",
        "category": "Media",
        "nips": [
            "F4"
        ],
        "description": "Use with the podcast\u2019s key",
        "items": [
            {
                "name": "Show metadata",
                "kinds": [
                    10154
                ],
                "default": true
            },
            {
                "name": "Episodes",
                "kinds": [
                    54
                ],
                "default": true
            }
        ],
        "notes": [
            "Use with the podcast\u2019s key"
        ]
    },
    {
        "id": "podcastfollow",
        "name": "Podcast subscriptions",
        "category": "Collections",
        "nips": [
            "F4",
            "51"
        ],
        "description": "Favorite podcasts",
        "items": [
            {
                "name": "Favorite podcasts",
                "kinds": [
                    10054
                ],
                "default": true
            }
        ],
        "notes": [
            "Private list entries need separate self-only encryption and decryption access."
        ]
    },
    {
        "id": "calendar",
        "name": "Calendar publishing",
        "category": "Events and activities",
        "nips": [
            "52"
        ],
        "description": "Date events, Timed events, Calendars",
        "items": [
            {
                "name": "Date events",
                "kinds": [
                    31922
                ],
                "default": true
            },
            {
                "name": "Timed events",
                "kinds": [
                    31923
                ],
                "default": true
            },
            {
                "name": "Calendars",
                "kinds": [
                    31924
                ],
                "default": true
            }
        ],
        "notes": []
    },
    {
        "id": "rsvp",
        "name": "Calendar RSVPs",
        "category": "Events and activities",
        "nips": [
            "52"
        ],
        "description": "RSVPs",
        "items": [
            {
                "name": "RSVPs",
                "kinds": [
                    31925
                ],
                "default": true
            }
        ],
        "notes": []
    },
    {
        "id": "pollcreate",
        "name": "Create polls",
        "category": "Events and activities",
        "nips": [
            "88"
        ],
        "description": "Polls",
        "items": [
            {
                "name": "Polls",
                "kinds": [
                    1068
                ],
                "default": true
            }
        ],
        "notes": []
    },
    {
        "id": "pollvote",
        "name": "Vote in polls",
        "category": "Events and activities",
        "nips": [
            "88"
        ],
        "description": "Poll responses",
        "items": [
            {
                "name": "Poll responses",
                "kinds": [
                    1018
                ],
                "default": true
            }
        ],
        "notes": []
    },
    {
        "id": "chess",
        "name": "Chess games",
        "category": "Events and activities",
        "nips": [
            "64"
        ],
        "description": "Chess game records",
        "items": [
            {
                "name": "Chess game records",
                "kinds": [
                    64
                ],
                "default": true
            }
        ],
        "notes": []
    },
    {
        "id": "cache",
        "name": "Geocaching",
        "category": "Events and activities",
        "nips": [
            "CC"
        ],
        "description": "Cache verification signatures use a separate cache key",
        "items": [
            {
                "name": "Cache listings",
                "kinds": [
                    37516
                ],
                "default": true
            },
            {
                "name": "Cache collections",
                "kinds": [
                    37517
                ],
                "default": true
            },
            {
                "name": "Found logs",
                "kinds": [
                    7516
                ],
                "default": true
            },
            {
                "name": "Comments",
                "kinds": [
                    1111
                ],
                "default": true
            }
        ],
        "notes": [
            "Cache verification signatures use a separate cache key"
        ]
    },
    {
        "id": "wiki",
        "name": "Wiki editing",
        "category": "Writing and development",
        "nips": [
            "54"
        ],
        "description": "Wiki articles, Redirects, Merge proposals",
        "items": [
            {
                "name": "Wiki articles",
                "kinds": [
                    30818
                ],
                "default": true
            },
            {
                "name": "Redirects",
                "kinds": [
                    30819
                ],
                "default": true
            },
            {
                "name": "Merge proposals",
                "kinds": [
                    818
                ],
                "default": true
            }
        ],
        "notes": []
    },
    {
        "id": "wikitrust",
        "name": "Wiki source preferences",
        "category": "Writing and development",
        "nips": [
            "54",
            "51"
        ],
        "description": "Preferred authors, Preferred relays",
        "items": [
            {
                "name": "Preferred authors",
                "kinds": [
                    10101
                ],
                "default": true
            },
            {
                "name": "Preferred relays",
                "kinds": [
                    10102
                ],
                "default": true
            }
        ],
        "notes": [
            "Private list entries need separate self-only encryption and decryption access."
        ]
    },
    {
        "id": "highlights",
        "name": "Text highlights",
        "category": "Writing and development",
        "nips": [
            "84"
        ],
        "description": "Highlights",
        "items": [
            {
                "name": "Highlights",
                "kinds": [
                    9802
                ],
                "default": true
            }
        ],
        "notes": []
    },
    {
        "id": "webbookmarks",
        "name": "Web bookmarks",
        "category": "Writing and development",
        "nips": [
            "B0"
        ],
        "description": "Web bookmarks",
        "items": [
            {
                "name": "Web bookmarks",
                "kinds": [
                    39701
                ],
                "default": true
            }
        ],
        "notes": []
    },
    {
        "id": "snippets",
        "name": "Code snippets",
        "category": "Writing and development",
        "nips": [
            "C0"
        ],
        "description": "Code snippets",
        "items": [
            {
                "name": "Code snippets",
                "kinds": [
                    1337
                ],
                "default": true
            }
        ],
        "notes": []
    },
    {
        "id": "gitcontrib",
        "name": "Git contributions",
        "category": "Writing and development",
        "nips": [
            "34"
        ],
        "description": "Patches, Pull requests, Pull request updates, Issues, Comments",
        "items": [
            {
                "name": "Patches",
                "kinds": [
                    1617
                ],
                "default": true
            },
            {
                "name": "Pull requests",
                "kinds": [
                    1618
                ],
                "default": true
            },
            {
                "name": "Pull request updates",
                "kinds": [
                    1619
                ],
                "default": true
            },
            {
                "name": "Issues",
                "kinds": [
                    1621
                ],
                "default": true
            },
            {
                "name": "Comments",
                "kinds": [
                    1111
                ],
                "default": true
            }
        ],
        "notes": []
    },
    {
        "id": "gitmaint",
        "name": "Git repository management",
        "category": "Writing and development",
        "nips": [
            "34"
        ],
        "description": "Repository announcement, Repository state, Open status, Applied status, Closed status, Draft status",
        "items": [
            {
                "name": "Repository announcement",
                "kinds": [
                    30617
                ],
                "default": true
            },
            {
                "name": "Repository state",
                "kinds": [
                    30618
                ],
                "default": true
            },
            {
                "name": "Open status",
                "kinds": [
                    1630
                ],
                "default": true
            },
            {
                "name": "Applied status",
                "kinds": [
                    1631
                ],
                "default": true
            },
            {
                "name": "Closed status",
                "kinds": [
                    1632
                ],
                "default": true
            },
            {
                "name": "Draft status",
                "kinds": [
                    1633
                ],
                "default": true
            }
        ],
        "notes": []
    },
    {
        "id": "gitprefs",
        "name": "Git preferences",
        "category": "Writing and development",
        "nips": [
            "34",
            "51"
        ],
        "description": "Git servers, Followed authors, Followed repositories",
        "items": [
            {
                "name": "Git servers",
                "kinds": [
                    10317
                ],
                "default": true
            },
            {
                "name": "Followed authors",
                "kinds": [
                    10017
                ],
                "default": true
            },
            {
                "name": "Followed repositories",
                "kinds": [
                    10018
                ],
                "default": true
            }
        ],
        "notes": [
            "Private list entries need separate self-only encryption and decryption access."
        ]
    },
    {
        "id": "releases",
        "name": "Release artifact collections",
        "category": "Writing and development",
        "nips": [
            "51",
            "94"
        ],
        "description": "Release artifact sets, File metadata",
        "items": [
            {
                "name": "Release artifact sets",
                "kinds": [
                    30063
                ],
                "default": true
            },
            {
                "name": "File metadata",
                "kinds": [
                    1063
                ],
                "default": true
            }
        ],
        "notes": [
            "Private list entries need separate self-only encryption and decryption access."
        ]
    },
    {
        "id": "labels",
        "name": "Labels",
        "category": "Discovery and trust",
        "nips": [
            "32"
        ],
        "description": "Content labels",
        "items": [
            {
                "name": "Content labels",
                "kinds": [
                    1985
                ],
                "default": true
            }
        ],
        "notes": []
    },
    {
        "id": "reports",
        "name": "Report content",
        "category": "Discovery and trust",
        "nips": [
            "56"
        ],
        "description": "Reports",
        "items": [
            {
                "name": "Reports",
                "kinds": [
                    1984
                ],
                "default": true
            }
        ],
        "notes": []
    },
    {
        "id": "badgedisplay",
        "name": "Display badges",
        "category": "Discovery and trust",
        "nips": [
            "58",
            "51"
        ],
        "description": "Profile badges, Badge collections",
        "items": [
            {
                "name": "Profile badges",
                "kinds": [
                    10008
                ],
                "default": true
            },
            {
                "name": "Badge collections",
                "kinds": [
                    30008
                ],
                "default": true
            }
        ],
        "notes": [
            "Private list entries need separate self-only encryption and decryption access."
        ]
    },
    {
        "id": "badgeissue",
        "name": "Issue badges",
        "category": "Discovery and trust",
        "nips": [
            "58"
        ],
        "description": "Badge definitions, Badge awards",
        "items": [
            {
                "name": "Badge definitions",
                "kinds": [
                    30009
                ],
                "default": true
            },
            {
                "name": "Badge awards",
                "kinds": [
                    8
                ],
                "default": true
            }
        ],
        "notes": []
    },
    {
        "id": "apprecommend",
        "name": "App recommendations",
        "category": "Discovery and trust",
        "nips": [
            "89",
            "51"
        ],
        "description": "Recommended handlers, App collections",
        "items": [
            {
                "name": "Recommended handlers",
                "kinds": [
                    31989
                ],
                "default": true
            },
            {
                "name": "App collections",
                "kinds": [
                    30267
                ],
                "default": true
            }
        ],
        "notes": [
            "Private list entries need separate self-only encryption and decryption access."
        ]
    },
    {
        "id": "apphandler",
        "name": "Publish app handlers",
        "category": "Discovery and trust",
        "nips": [
            "89"
        ],
        "description": "App handler metadata",
        "items": [
            {
                "name": "App handler metadata",
                "kinds": [
                    31990
                ],
                "default": true
            }
        ],
        "notes": []
    },
    {
        "id": "trustsources",
        "name": "Trust provider preferences",
        "category": "Discovery and trust",
        "nips": [
            "85"
        ],
        "description": "Trusted assertion providers",
        "items": [
            {
                "name": "Trusted assertion providers",
                "kinds": [
                    10040
                ],
                "default": true
            }
        ],
        "notes": []
    },
    {
        "id": "assertions",
        "name": "Publish trust assertions",
        "category": "Advanced roles",
        "nips": [
            "85"
        ],
        "description": "For a trust provider identity",
        "items": [
            {
                "name": "User assertions",
                "kinds": [
                    30382
                ],
                "default": true
            },
            {
                "name": "Event assertions",
                "kinds": [
                    30383
                ],
                "default": true
            },
            {
                "name": "Address assertions",
                "kinds": [
                    30384
                ],
                "default": true
            },
            {
                "name": "External identifier assertions",
                "kinds": [
                    30385
                ],
                "default": true
            }
        ],
        "notes": [
            "For a trust provider identity"
        ]
    },
    {
        "id": "classifieds",
        "name": "Classified listings",
        "category": "Commerce",
        "nips": [
            "99"
        ],
        "description": "Listings, Listing drafts",
        "items": [
            {
                "name": "Listings",
                "kinds": [
                    30402
                ],
                "default": true
            },
            {
                "name": "Listing drafts",
                "kinds": [
                    30403
                ],
                "default": true
            }
        ],
        "notes": []
    },
    {
        "id": "goals",
        "name": "Fundraising goals",
        "category": "Commerce",
        "nips": [
            "75"
        ],
        "description": "Fundraising goals",
        "items": [
            {
                "name": "Fundraising goals",
                "kinds": [
                    9041
                ],
                "default": true
            }
        ],
        "notes": []
    },
    {
        "id": "zaps",
        "name": "Send zap requests",
        "category": "Commerce",
        "nips": [
            "57"
        ],
        "description": "Receipts are signed by the payment service",
        "items": [
            {
                "name": "Zap requests",
                "kinds": [
                    9734
                ],
                "default": true
            }
        ],
        "notes": []
    },
    {
        "id": "paymenttargets",
        "name": "Payment preferences",
        "category": "Commerce",
        "nips": [
            "A3"
        ],
        "description": "Payment targets",
        "items": [
            {
                "name": "Payment targets",
                "kinds": [
                    10133
                ],
                "default": true
            }
        ],
        "notes": []
    },
    {
        "id": "mintrecommend",
        "name": "Mint recommendations",
        "category": "Commerce",
        "nips": [
            "87"
        ],
        "description": "Mint recommendations",
        "items": [
            {
                "name": "Mint recommendations",
                "kinds": [
                    38000
                ],
                "default": true
            }
        ],
        "notes": []
    },
    {
        "id": "p2p",
        "name": "Peer-to-peer order announcements",
        "category": "Commerce",
        "nips": [
            "69"
        ],
        "description": "Choose the key used for the order protocol",
        "items": [
            {
                "name": "Order announcements",
                "kinds": [
                    38383
                ],
                "default": true
            }
        ],
        "notes": [
            "Choose the key used for the order protocol"
        ]
    },
    {
        "id": "torrent",
        "name": "Torrent publishing",
        "category": "Media",
        "nips": [
            "35"
        ],
        "description": "Torrent metadata, Torrent comments",
        "items": [
            {
                "name": "Torrent metadata",
                "kinds": [
                    2003
                ],
                "default": true
            },
            {
                "name": "Torrent comments",
                "kinds": [
                    2004
                ],
                "default": true
            }
        ],
        "notes": []
    },
    {
        "id": "filesystem",
        "name": "File storage indexes",
        "category": "Writing and development",
        "nips": [
            "5A"
        ],
        "description": "Root index, Named indexes, Snapshots, Legacy index",
        "items": [
            {
                "name": "Root index",
                "kinds": [
                    15128
                ],
                "default": true
            },
            {
                "name": "Named indexes",
                "kinds": [
                    35128
                ],
                "default": true
            },
            {
                "name": "Snapshots",
                "kinds": [
                    5128
                ],
                "default": true
            },
            {
                "name": "Legacy index",
                "kinds": [
                    34128
                ],
                "default": false
            }
        ],
        "notes": []
    },
    {
        "id": "appdata",
        "name": "Application data",
        "category": "Advanced roles",
        "nips": [
            "78"
        ],
        "description": "Applies across app namespaces; cannot restrict an individual app",
        "items": [
            {
                "name": "Application data",
                "kinds": [
                    78
                ],
                "default": true
            },
            {
                "name": "Addressable application data",
                "kinds": [
                    30078
                ],
                "default": true
            }
        ],
        "notes": [
            "Applies across app namespaces; cannot restrict an individual app"
        ]
    },
    {
        "id": "httpauth",
        "name": "HTTP authentication",
        "category": "Authentication",
        "nips": [
            "98",
            "86"
        ],
        "description": "Can authorize HTTP requests; no URL or method restriction in this policy",
        "items": [
            {
                "name": "Sign HTTP authentication",
                "kinds": [
                    27235
                ],
                "default": true
            }
        ],
        "notes": [
            "Can authorize HTTP requests; no URL or method restriction in this policy"
        ]
    },
    {
        "id": "relayauth",
        "name": "Relay authentication",
        "category": "Authentication",
        "nips": [
            "42",
            "70"
        ],
        "description": "For the connecting client\u2019s authentication requests",
        "items": [
            {
                "name": "Sign relay authentication",
                "kinds": [
                    22242
                ],
                "default": true
            }
        ],
        "notes": []
    },
    {
        "id": "relaymembership",
        "name": "Relay membership requests",
        "category": "Authentication",
        "nips": [
            "43"
        ],
        "description": "Join relay, Leave relay",
        "items": [
            {
                "name": "Join relay",
                "kinds": [
                    28934
                ],
                "default": true
            },
            {
                "name": "Leave relay",
                "kinds": [
                    28936
                ],
                "default": true
            }
        ],
        "notes": []
    },
    {
        "id": "delete",
        "name": "Delete events",
        "category": "Advanced roles",
        "nips": [
            "09"
        ],
        "description": "Applies across event types; cannot limit deletion to one use case",
        "items": [
            {
                "name": "Request event deletion",
                "kinds": [
                    5
                ],
                "default": true
            }
        ],
        "notes": [
            "Applies across event types; cannot limit deletion to one use case"
        ]
    },
    {
        "id": "vanish",
        "name": "Request account removal",
        "category": "Advanced roles",
        "nips": [
            "62"
        ],
        "description": "Request removal from relays",
        "items": [
            {
                "name": "Request removal from relays",
                "kinds": [
                    62
                ],
                "default": true
            }
        ],
        "notes": []
    },
    {
        "id": "groupadmin",
        "name": "Group moderation",
        "category": "Advanced roles",
        "nips": [
            "29"
        ],
        "description": "Add member or change roles, Remove member, Change metadata, Delete event, Create group, Delete group, Create invitation, Update pinned events",
        "items": [
            {
                "name": "Add member or change roles",
                "kinds": [
                    9000
                ],
                "default": true
            },
            {
                "name": "Remove member",
                "kinds": [
                    9001
                ],
                "default": true
            },
            {
                "name": "Change metadata",
                "kinds": [
                    9002
                ],
                "default": true
            },
            {
                "name": "Delete event",
                "kinds": [
                    9005
                ],
                "default": true
            },
            {
                "name": "Create group",
                "kinds": [
                    9007
                ],
                "default": true
            },
            {
                "name": "Delete group",
                "kinds": [
                    9008
                ],
                "default": true
            },
            {
                "name": "Create invitation",
                "kinds": [
                    9009
                ],
                "default": true
            },
            {
                "name": "Update pinned events",
                "kinds": [
                    9010
                ],
                "default": true
            }
        ],
        "notes": []
    },
    {
        "id": "monitor",
        "name": "Relay monitoring",
        "category": "Advanced roles",
        "nips": [
            "66"
        ],
        "description": "For a relay monitoring identity",
        "items": [
            {
                "name": "Monitor announcements",
                "kinds": [
                    10166
                ],
                "default": true
            },
            {
                "name": "Relay discovery",
                "kinds": [
                    30166
                ],
                "default": true
            }
        ],
        "notes": [
            "For a relay monitoring identity"
        ]
    },
    {
        "id": "legacydm",
        "name": "Legacy private messages",
        "category": "Direct messaging",
        "nips": [
            "04"
        ],
        "description": "Legacy NIP-04 encryption and decryption",
        "items": [
            {
                "name": "Encrypted messages",
                "kinds": [
                    4
                ],
                "default": true,
                "crypto": {
                    "nip04_encrypt": "any",
                    "nip04_decrypt": "any"
                }
            }
        ],
        "notes": []
    },
    {
        "id": "legacychat",
        "name": "Legacy public channels",
        "category": "Legacy compatibility",
        "nips": [
            "28",
            "51"
        ],
        "description": "Create channels, Channel metadata, Channel messages, Hide messages, Mute users, Channel list",
        "items": [
            {
                "name": "Create channels",
                "kinds": [
                    40
                ],
                "default": true
            },
            {
                "name": "Channel metadata",
                "kinds": [
                    41
                ],
                "default": true
            },
            {
                "name": "Channel messages",
                "kinds": [
                    42
                ],
                "default": true
            },
            {
                "name": "Hide messages",
                "kinds": [
                    43
                ],
                "default": true
            },
            {
                "name": "Mute users",
                "kinds": [
                    44
                ],
                "default": true
            },
            {
                "name": "Channel list",
                "kinds": [
                    10005
                ],
                "default": true
            }
        ],
        "notes": [
            "Private list entries need separate self-only encryption and decryption access."
        ]
    },
    {
        "id": "legacycommunity",
        "name": "Legacy communities",
        "category": "Legacy compatibility",
        "nips": [
            "72",
            "51"
        ],
        "description": "Community definitions, Post approvals, Community list",
        "items": [
            {
                "name": "Community definitions",
                "kinds": [
                    34550
                ],
                "default": true
            },
            {
                "name": "Post approvals",
                "kinds": [
                    4550
                ],
                "default": true
            },
            {
                "name": "Community list",
                "kinds": [
                    10004
                ],
                "default": true
            }
        ],
        "notes": [
            "Private list entries need separate self-only encryption and decryption access."
        ]
    },
    {
        "id": "legacymarket",
        "name": "Legacy marketplace",
        "category": "Legacy compatibility",
        "nips": [
            "15"
        ],
        "description": "NIP-15 is unrecommended; messaging and deletion are separate",
        "items": [
            {
                "name": "Stalls",
                "kinds": [
                    30017
                ],
                "default": true
            },
            {
                "name": "Products",
                "kinds": [
                    30018
                ],
                "default": true
            }
        ],
        "notes": []
    },
    {
        "id": "legacyfiles",
        "name": "Legacy file server preferences",
        "category": "Legacy compatibility",
        "nips": [
            "96"
        ],
        "description": "NIP-96 is superseded by Blossom; HTTP authorization is separate",
        "items": [
            {
                "name": "File server preferences",
                "kinds": [
                    10096
                ],
                "default": true
            }
        ],
        "notes": []
    }
];
