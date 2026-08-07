# Compatibility matrix — last run

Produced by `pwsh -File docs/testing/run-matrix.ps1`. Re-run it after any change to the read,
replace or layout-switch paths; a fix that trades one application for another shows up here and
nowhere else.

Run 2026-08-07 against the release build at commit `ee84d85`, with all three layouts installed
(en-US, de-DE, uk-UA).

**16 of 16 pass.**

| Application | Initial text                                     | Actions                             | Layout before | Expected text                                    | Actual text                                      | Expected layout | Actual layout | Pass |
|-------------|--------------------------------------------------|-------------------------------------|---------------|--------------------------------------------------|--------------------------------------------------|-----------------|---------------|------|
| Notepad     | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | select all, Shift+Pause, Enter      | en-US         | `Хочу дещо в тебе запитати. От дивись ви зроби…` | `Хочу дещо в тебе запитати. От дивись ви зроби…` | en-US           | en-US         | PASS |
| Notepad     | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | select all, Shift+Pause, Ctrl+Enter | en-US         | `Хочу дещо в тебе запитати. От дивись ви зроби…` | `Хочу дещо в тебе запитати. От дивись ви зроби…` | uk-UA           | uk-UA         | PASS |
| Notepad     | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | select all, Shift+Pause, Esc        | en-US         | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | en-US           | en-US         | PASS |
| Notepad     | `1234567890`                                     | select all, Shift+Pause, Esc        | en-US         | `1234567890`                                     | `1234567890`                                     | en-US           | en-US         | PASS |
| Notepad++   | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | select all, Shift+Pause, Enter      | en-US         | `Хочу дещо в тебе запитати. От дивись ви зроби…` | `Хочу дещо в тебе запитати. От дивись ви зроби…` | en-US           | en-US         | PASS |
| Notepad++   | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | select all, Shift+Pause, Ctrl+Enter | en-US         | `Хочу дещо в тебе запитати. От дивись ви зроби…` | `Хочу дещо в тебе запитати. От дивись ви зроби…` | uk-UA           | uk-UA         | PASS |
| Notepad++   | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | select all, Shift+Pause, Esc        | en-US         | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | en-US           | en-US         | PASS |
| Notepad++   | `1234567890`                                     | select all, Shift+Pause, Esc        | en-US         | `1234567890`                                     | `1234567890`                                     | en-US           | en-US         | PASS |
| Chrome      | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | select all, Shift+Pause, Enter      | en-US         | `Хочу дещо в тебе запитати. От дивись ви зроби…` | `Хочу дещо в тебе запитати. От дивись ви зроби…` | en-US           | en-US         | PASS |
| Chrome      | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | select all, Shift+Pause, Ctrl+Enter | en-US         | `Хочу дещо в тебе запитати. От дивись ви зроби…` | `Хочу дещо в тебе запитати. От дивись ви зроби…` | uk-UA           | uk-UA         | PASS |
| Chrome      | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | select all, Shift+Pause, Esc        | en-US         | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | en-US           | en-US         | PASS |
| Chrome      | `1234567890`                                     | select all, Shift+Pause, Esc        | en-US         | `1234567890`                                     | `1234567890`                                     | en-US           | en-US         | PASS |
| Edge        | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | select all, Shift+Pause, Enter      | en-US         | `Хочу дещо в тебе запитати. От дивись ви зроби…` | `Хочу дещо в тебе запитати. От дивись ви зроби…` | en-US           | en-US         | PASS |
| Edge        | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | select all, Shift+Pause, Ctrl+Enter | en-US         | `Хочу дещо в тебе запитати. От дивись ви зроби…` | `Хочу дещо в тебе запитати. От дивись ви зроби…` | uk-UA           | uk-UA         | PASS |
| Edge        | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | select all, Shift+Pause, Esc        | en-US         | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | `{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,b…` | en-US           | en-US         | PASS |
| Edge        | `1234567890`                                     | select all, Shift+Pause, Esc        | en-US         | `1234567890`                                     | `1234567890`                                     | en-US           | en-US         | PASS |

