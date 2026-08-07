# Compatibility matrix — last run

Produced by `pwsh -File docs/testing/run-matrix.ps1`. Re-run after any change to the read,
replace or layout-switch paths.

Run 2026-08-07 against the release build at commit `e3c88ad`. **25 of 25 pass** (two `n/a`: a native
editor has no read-only field).

Windows Terminal is out of scope and VS Code is not installed here — see the archived
`add-candidate-chooser` task 7.8.

| Application | Initial text | Actions | Layout before | Expected text | Actual text | Expected layout | Actual layout | Pass |
|---|---|---|---|---|---|---|---|---|
| Notepad | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | select all, Shift+Pause, Enter | en-US | `Хочу дещо в тебе запитати. От дивись ви зроби…` | `Хочу дещо в тебе запитати. От дивись ви зроби…` | en-US | en-US | PASS |
| Notepad | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | select all, Shift+Pause, Ctrl+Enter | en-US | `Хочу дещо в тебе запитати. От дивись ви зроби…` | `Хочу дещо в тебе запитати. От дивись ви зроби…` | uk-UA | uk-UA | PASS |
| Notepad | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | select all, Shift+Pause, Esc | en-US | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | en-US | en-US | PASS |
| Notepad | `1234567890` | select all, Shift+Pause, Esc | en-US | `1234567890` | `1234567890` | en-US | en-US | PASS |
| Notepad | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | select all in a read-only field, Shift+Pause, Enter | en-US | `n/a` | `n/a` | n/a | n/a | PASS |
| Notepad | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | deselect, Shift+Pause | en-US | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | en-US | en-US | PASS |
| Notepad++ | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | select all, Shift+Pause, Enter | en-US | `Хочу дещо в тебе запитати. От дивись ви зроби…` | `Хочу дещо в тебе запитати. От дивись ви зроби…` | en-US | en-US | PASS |
| Notepad++ | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | select all, Shift+Pause, Ctrl+Enter | en-US | `Хочу дещо в тебе запитати. От дивись ви зроби…` | `Хочу дещо в тебе запитати. От дивись ви зроби…` | uk-UA | uk-UA | PASS |
| Notepad++ | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | select all, Shift+Pause, Esc | en-US | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | en-US | en-US | PASS |
| Notepad++ | `1234567890` | select all, Shift+Pause, Esc | en-US | `1234567890` | `1234567890` | en-US | en-US | PASS |
| Notepad++ | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | select all in a read-only field, Shift+Pause, Enter | en-US | `n/a` | `n/a` | n/a | n/a | PASS |
| Notepad++ | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | deselect, Shift+Pause | en-US | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | en-US | en-US | PASS |
| Chrome | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | select all, Shift+Pause, Enter | en-US | `Хочу дещо в тебе запитати. От дивись ви зроби…` | `Хочу дещо в тебе запитати. От дивись ви зроби…` | en-US | en-US | PASS |
| Chrome | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | select all, Shift+Pause, Ctrl+Enter | en-US | `Хочу дещо в тебе запитати. От дивись ви зроби…` | `Хочу дещо в тебе запитати. От дивись ви зроби…` | uk-UA | uk-UA | PASS |
| Chrome | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | select all, Shift+Pause, Esc | en-US | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | en-US | en-US | PASS |
| Chrome | `1234567890` | select all, Shift+Pause, Esc | en-US | `1234567890` | `1234567890` | en-US | en-US | PASS |
| Chrome | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | select all in a read-only field, Shift+Pause, Enter | en-US | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | en-US | en-US | PASS |
| Chrome | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | deselect, Shift+Pause | en-US | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | en-US | en-US | PASS |
| Edge | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | select all, Shift+Pause, Enter | en-US | `Хочу дещо в тебе запитати. От дивись ви зроби…` | `Хочу дещо в тебе запитати. От дивись ви зроби…` | en-US | en-US | PASS |
| Edge | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | select all, Shift+Pause, Ctrl+Enter | en-US | `Хочу дещо в тебе запитати. От дивись ви зроби…` | `Хочу дещо в тебе запитати. От дивись ви зроби…` | uk-UA | uk-UA | PASS |
| Edge | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | select all, Shift+Pause, Esc | en-US | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | en-US | en-US | PASS |
| Edge | `1234567890` | select all, Shift+Pause, Esc | en-US | `1234567890` | `1234567890` | en-US | en-US | PASS |
| Edge | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | select all in a read-only field, Shift+Pause, Enter | en-US | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | en-US | en-US | PASS |
| Edge | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | deselect, Shift+Pause | en-US | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | en-US | en-US | PASS |

Notes:
- Notepad / select all in a read-only field, Shift+Pause, Enter: not applicable: needs a read-only field
- Notepad / deselect, Shift+Pause: no chooser appeared
- Notepad++ / select all in a read-only field, Shift+Pause, Enter: not applicable: needs a read-only field
- Chrome / deselect, Shift+Pause: no chooser appeared
- Edge / deselect, Shift+Pause: no chooser appeared
