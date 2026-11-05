# Parts and Macros follow-ups, QA at exact head

Source at git 598733481c (code head). Sizes 1366x768 and 1920x1080. 702 passes at both sizes (S4 overlap notes are non-blocking; baseline against clean main not run); 7432 passes at both (was failing at 1920); 7436 passes at both.

## 702-parts-library
| Parts: a library bundle is opened, its parts are added, and the user's own parts are exported (702-parts-library-bundle) | Parts | 1366x768 | pass | F-001, F-002, F-003 | 10 |
| Parts: a library bundle is opened, its parts are added, and the user's own parts are exported (702-parts-library-bundle) | Parts | 1920x1080 | pass | F-004, F-005, F-006 | 10 |

## 7432-help-icons-parts
| Help icons: Parts and Macros carry no explanation text (7432-help-icons-parts-macros) | Parts | 1366x768 | pass | - | 2 |
| Help icons: Parts and Macros carry no explanation text (7432-help-icons-parts-macros) | Parts | 1920x1080 | pass | - | 2 |

## 7436
| Place macro: the (suggestion) tag of the mounting view is on screen, folded and open (7436-place-macro-suggestion-tag) | Parts | 1366x768 | pass | - | 2 |
| Place macro: the (suggestion) tag of the mounting view is on screen, folded and open (7436-place-macro-suggestion-tag) | Parts | 1920x1080 | pass | - | 2 |

Not run here: 1702-describe-devices-recipe fails with an unrelated text miss.
