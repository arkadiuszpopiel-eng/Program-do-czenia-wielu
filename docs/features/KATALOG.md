# Katalog funkcji i testów

Plik generowany: `node scripts/features.mjs` (nie edytuj ręcznie). Zasady i system testów:
[45-testy-ai.md](../plan/45-testy-ai.md). Ślad = identyfikator funkcji w kodzie testu danego poziomu;
„bieg aplikacji” = polecenie wywołane w ostatnim biegu prawdziwej aplikacji na tej maszynie (tylko informacja).

**Razem: 603 funkcji, z testem 603 (100 %), w długu 0.**

| Rodzaj | Funkcji | Z testem | rdzeń | interfejs | przeglądarka | aplikacja | eval | przegląd ogólny |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Polecenia rdzenia | 283 | 283 (100 %) | 206 | 3 | 2 | 271 | 1 | 0 |
| Polecenia palety i skróty | 62 | 62 (100 %) | 2 | 7 | 9 | 3 | 0 | 50 |
| Strony Ustawień | 38 | 38 (100 %) | 0 | 3 | 34 | 12 | 0 | 38 |
| Ustawienia | 98 | 98 (100 %) | 14 | 12 | 7 | 9 | 2 | 98 |
| Narzędzia agentek | 81 | 81 (100 %) | 81 | 1 | 1 | 3 | 52 | 81 |
| Zdarzenia rdzenia | 41 | 41 (100 %) | 35 | 25 | 5 | 1 | 1 | 41 |

## Polecenia rdzenia

| Funkcja | Testy | Uwagi |
| --- | --- | --- |
| `accounts_add` | rdzeń, aplikacja, bieg aplikacji |  |
| `accounts_assign` | rdzeń, aplikacja, bieg aplikacji |  |
| `accounts_catalog` | rdzeń, aplikacja, bieg aplikacji |  |
| `accounts_import_env` | aplikacja, bieg aplikacji |  |
| `accounts_list` | rdzeń, aplikacja, bieg aplikacji |  |
| `accounts_remove` | rdzeń, aplikacja, bieg aplikacji |  |
| `accounts_rotate` | rdzeń, aplikacja, bieg aplikacji |  |
| `accounts_set_disabled` | rdzeń, aplikacja, bieg aplikacji |  |
| `accounts_set_enterprise` | rdzeń, aplikacja |  |
| `accounts_set_limit` | rdzeń, aplikacja, bieg aplikacji |  |
| `accounts_set_model` | rdzeń, aplikacja, bieg aplikacji |  |
| `accounts_set_queue` | rdzeń, aplikacja, bieg aplikacji |  |
| `accounts_set_shared_queue` | rdzeń, aplikacja, bieg aplikacji |  |
| `accounts_shared_queues` | rdzeń, aplikacja, bieg aplikacji |  |
| `accounts_suggest_queue` | rdzeń, aplikacja, bieg aplikacji |  |
| `accounts_test` | rdzeń, aplikacja, bieg aplikacji |  |
| `agents_apply_cast` | rdzeń, aplikacja, bieg aplikacji |  |
| `agents_list` | rdzeń, aplikacja, bieg aplikacji |  |
| `agents_open_terminal` | rdzeń, aplikacja, bieg aplikacji |  |
| `agents_runs` | rdzeń, aplikacja, bieg aplikacji |  |
| `agents_set_roles` | rdzeń, aplikacja, bieg aplikacji |  |
| `agents_steer` | rdzeń, aplikacja, bieg aplikacji |  |
| `app_bootstrap` | rdzeń, przeglądarka, aplikacja, bieg aplikacji |  |
| `app_complete_onboarding` | rdzeń, aplikacja, bieg aplikacji |  |
| `app_open_system_settings` | rdzeń, aplikacja, bieg aplikacji |  |
| `app_save_layout` | rdzeń, aplikacja, bieg aplikacji |  |
| `app_set_active_session` | rdzeń, aplikacja, bieg aplikacji |  |
| `attachments_add_camera` | rdzeń, aplikacja, bieg aplikacji |  |
| `attachments_add_dropped` | rdzeń, aplikacja, bieg aplikacji |  |
| `attachments_list` | rdzeń, bieg aplikacji |  |
| `attachments_paste` | rdzeń, aplikacja, bieg aplikacji |  |
| `attachments_pick` | rdzeń, aplikacja, bieg aplikacji |  |
| `attachments_remove` | rdzeń, aplikacja, bieg aplikacji |  |
| `backups_choose_dir` | rdzeń, aplikacja |  |
| `backups_configure` | rdzeń, aplikacja, bieg aplikacji |  |
| `backups_restore` | rdzeń, aplikacja, bieg aplikacji |  |
| `backups_run_now` | rdzeń, aplikacja, bieg aplikacji |  |
| `backups_set_password` | rdzeń, aplikacja, bieg aplikacji |  |
| `backups_status` | rdzeń, aplikacja, bieg aplikacji |  |
| `backups_verify` | rdzeń, aplikacja, bieg aplikacji |  |
| `board_list` | aplikacja, bieg aplikacji |  |
| `board_pin` | aplikacja, bieg aplikacji |  |
| `board_unpin` | aplikacja, bieg aplikacji |  |
| `bridges_check` | rdzeń, aplikacja, bieg aplikacji |  |
| `bridges_list` | rdzeń, aplikacja, bieg aplikacji |  |
| `bridges_mark_login` | rdzeń, aplikacja, bieg aplikacji |  |
| `bridges_open_login` | rdzeń, aplikacja, bieg aplikacji |  |
| `bridges_pin` | rdzeń, aplikacja, bieg aplikacji |  |
| `bridges_set_enabled` | aplikacja, bieg aplikacji |  |
| `bridges_set_schedule` | rdzeń, aplikacja, bieg aplikacji |  |
| `broker_status` | rdzeń, aplikacja, bieg aplikacji |  |
| `builder_dry_run` | rdzeń, aplikacja, bieg aplikacji |  |
| `builder_library` | rdzeń, aplikacja, bieg aplikacji |  |
| `builder_policy` | rdzeń, aplikacja, bieg aplikacji |  |
| `builder_preview` | rdzeń, aplikacja, bieg aplikacji |  |
| `builder_propose` | aplikacja, bieg aplikacji |  |
| `builder_save` | rdzeń, aplikacja, bieg aplikacji |  |
| `builder_voice_preview` | rdzeń, aplikacja |  |
| `cameras_frame` | rdzeń, aplikacja, bieg aplikacji |  |
| `cameras_refresh` | rdzeń, aplikacja, bieg aplikacji |  |
| `cameras_rename` | rdzeń, aplikacja, bieg aplikacji |  |
| `cameras_set_enabled` | rdzeń, aplikacja, bieg aplikacji |  |
| `cameras_set_master` | rdzeń, aplikacja, bieg aplikacji |  |
| `cameras_state` | rdzeń, aplikacja, bieg aplikacji |  |
| `canvas_render` | rdzeń, aplikacja |  |
| `compliance_routes` | aplikacja, bieg aplikacji |  |
| `config_raw_check` | rdzeń, aplikacja, bieg aplikacji |  |
| `config_raw_get` | rdzeń, aplikacja, bieg aplikacji |  |
| `config_raw_save` | rdzeń, aplikacja, bieg aplikacji |  |
| `costs_set_monthly_limit` | rdzeń, aplikacja, bieg aplikacji |  |
| `costs_summary` | rdzeń, interfejs, aplikacja, bieg aplikacji |  |
| `device_gpu_live` | aplikacja, bieg aplikacji |  |
| `device_measure` | rdzeń, bieg aplikacji |  |
| `device_profile` | rdzeń, aplikacja, bieg aplikacji |  |
| `diag_cancel` | rdzeń, aplikacja, bieg aplikacji |  |
| `diag_export` | rdzeń, aplikacja, bieg aplikacji |  |
| `diag_run` | rdzeń, aplikacja, bieg aplikacji |  |
| `diag_runs` | rdzeń, aplikacja, bieg aplikacji |  |
| `diag_snapshot` | rdzeń, aplikacja, bieg aplikacji |  |
| `diag_state` | rdzeń, aplikacja, bieg aplikacji |  |
| `embed_model_activate` | rdzeń, aplikacja, bieg aplikacji |  |
| `evals_list` | aplikacja, bieg aplikacji |  |
| `evals_verify` | aplikacja, bieg aplikacji |  |
| `events_resume` | rdzeń, aplikacja, bieg aplikacji |  |
| `files_act` | aplikacja, bieg aplikacji |  |
| `files_list` | rdzeń, aplikacja, bieg aplikacji |  |
| `files_preview` | aplikacja, bieg aplikacji |  |
| `gui_desktop_grant` | aplikacja, bieg aplikacji |  |
| `gui_release` | rdzeń, aplikacja, bieg aplikacji |  |
| `gui_screenshot` | aplikacja, bieg aplikacji |  |
| `gui_status` | rdzeń, aplikacja, bieg aplikacji |  |
| `gui_stop` | rdzeń, aplikacja, bieg aplikacji |  |
| `health_approve` | rdzeń, aplikacja, bieg aplikacji |  |
| `health_reject` | aplikacja, bieg aplikacji |  |
| `health_report` | aplikacja, bieg aplikacji |  |
| `health_scan` | rdzeń, aplikacja, bieg aplikacji |  |
| `health_undo` | rdzeń, aplikacja, bieg aplikacji |  |
| `help_page` | aplikacja, bieg aplikacji |  |
| `help_search` | aplikacja, bieg aplikacji |  |
| `help_topics` | aplikacja, bieg aplikacji |  |
| `improver_approve` | rdzeń, aplikacja, bieg aplikacji |  |
| `improver_cycle` | aplikacja, bieg aplikacji |  |
| `improver_list` | rdzeń, aplikacja, bieg aplikacji |  |
| `improver_reject` | aplikacja, bieg aplikacji |  |
| `improver_rollback` | aplikacja, bieg aplikacji |  |
| `marshal_approve` | aplikacja, bieg aplikacji |  |
| `marshal_propose` | aplikacja, bieg aplikacji |  |
| `marshal_reject` | aplikacja, bieg aplikacji |  |
| `marshal_report` | aplikacja, bieg aplikacji |  |
| `marshal_revoke` | aplikacja, bieg aplikacji |  |
| `marshal_state` | aplikacja, bieg aplikacji |  |
| `mcp_approve` | rdzeń, aplikacja, bieg aplikacji |  |
| `mcp_list` | rdzeń, aplikacja, bieg aplikacji |  |
| `mcp_remove` | rdzeń, aplikacja, bieg aplikacji |  |
| `mcp_revoke` | aplikacja, bieg aplikacji |  |
| `mcp_save` | rdzeń, aplikacja, bieg aplikacji |  |
| `mcp_tools` | rdzeń, aplikacja, bieg aplikacji |  |
| `memory_approve` | aplikacja, bieg aplikacji |  |
| `memory_consolidate_now` | aplikacja, bieg aplikacji |  |
| `memory_edit` | aplikacja, bieg aplikacji |  |
| `memory_explain` | rdzeń, aplikacja, bieg aplikacji |  |
| `memory_forget` | aplikacja, bieg aplikacji |  |
| `memory_forget_preview` | rdzeń, aplikacja, bieg aplikacji |  |
| `memory_inspect` | rdzeń, aplikacja, bieg aplikacji |  |
| `memory_journal` | aplikacja, bieg aplikacji |  |
| `memory_promote` | aplikacja, bieg aplikacji |  |
| `memory_scopes` | rdzeń, aplikacja, bieg aplikacji |  |
| `memory_set_pinned` | aplikacja, bieg aplikacji |  |
| `memory_status` | aplikacja, bieg aplikacji |  |
| `memory_teach` | rdzeń, aplikacja |  |
| `memory_teach_pick_file` | rdzeń, aplikacja |  |
| `memory_undo` | aplikacja, bieg aplikacji |  |
| `models_bundle_download` | aplikacja, bieg aplikacji |  |
| `models_bundle_verify` | aplikacja, bieg aplikacji |  |
| `models_bundles` | rdzeń, aplikacja, bieg aplikacji |  |
| `models_cancel` | aplikacja, bieg aplikacji |  |
| `models_download` | rdzeń, aplikacja, bieg aplikacji |  |
| `models_list` | rdzeń, aplikacja, bieg aplikacji |  |
| `models_local_cancel` | aplikacja, bieg aplikacji |  |
| `models_local_download` | rdzeń, aplikacja, bieg aplikacji |  |
| `models_local_list` | rdzeń, aplikacja, bieg aplikacji |  |
| `models_remove` | aplikacja, bieg aplikacji |  |
| `models_repair` | rdzeń, aplikacja, bieg aplikacji |  |
| `models_trust_hash` | aplikacja, bieg aplikacji |  |
| `models_verify` | aplikacja, bieg aplikacji |  |
| `modules_list` | rdzeń, aplikacja, bieg aplikacji |  |
| `modules_set_enabled` | rdzeń, aplikacja, bieg aplikacji |  |
| `permissions_get` | rdzeń, aplikacja, bieg aplikacji |  |
| `permissions_grants` | rdzeń, aplikacja, bieg aplikacji |  |
| `permissions_open_approval` | rdzeń, aplikacja, bieg aplikacji |  |
| `permissions_request_level` | rdzeń, aplikacja, bieg aplikacji |  |
| `permissions_revoke_grant` | rdzeń, aplikacja, bieg aplikacji |  |
| `pipelines_list` | rdzeń, aplikacja |  |
| `pipelines_remove` | rdzeń, aplikacja |  |
| `pipelines_save` | rdzeń, aplikacja |  |
| `plugins_approve` | rdzeń, aplikacja, bieg aplikacji |  |
| `plugins_disable` | rdzeń, aplikacja, bieg aplikacji |  |
| `plugins_enable` | aplikacja, bieg aplikacji |  |
| `plugins_inspect` | rdzeń, aplikacja, bieg aplikacji |  |
| `plugins_list` | rdzeń, aplikacja, bieg aplikacji |  |
| `plugins_propose` | rdzeń, aplikacja, bieg aplikacji |  |
| `plugins_reject` | aplikacja, bieg aplikacji |  |
| `plugins_remove` | rdzeń, aplikacja, bieg aplikacji |  |
| `pricing_apply` | rdzeń, aplikacja |  |
| `pricing_get` | rdzeń, bieg aplikacji |  |
| `projects_list` | rdzeń, aplikacja |  |
| `projects_pick_dir` | rdzeń, aplikacja |  |
| `projects_remove` | rdzeń, aplikacja |  |
| `projects_rename` | rdzeń, aplikacja |  |
| `projects_save` | rdzeń, aplikacja |  |
| `prompt_templates_get` | rdzeń, aplikacja, bieg aplikacji |  |
| `prompt_templates_set` | rdzeń, aplikacja, bieg aplikacji |  |
| `quick_ask` | rdzeń, interfejs, przeglądarka, aplikacja, bieg aplikacji |  |
| `quick_commands_get` | rdzeń, aplikacja, bieg aplikacji |  |
| `quick_commands_set` | rdzeń, aplikacja, bieg aplikacji |  |
| `quick_commands_test` | rdzeń, aplikacja, bieg aplikacji |  |
| `quick_expand_to_main` | rdzeń, aplikacja, bieg aplikacji |  |
| `quick_hide` | rdzeń, aplikacja, bieg aplikacji |  |
| `routing_get` | rdzeń, aplikacja, bieg aplikacji |  |
| `routing_preview` | rdzeń, bieg aplikacji |  |
| `routing_set` | rdzeń, bieg aplikacji |  |
| `search_reindex_cancel` | rdzeń, aplikacja, bieg aplikacji |  |
| `search_reindex_start` | aplikacja, bieg aplikacji |  |
| `search_reindex_status` | rdzeń, aplikacja, bieg aplikacji |  |
| `sessions_cards` | rdzeń, aplikacja |  |
| `sessions_choose_workdir` | rdzeń, aplikacja, bieg aplikacji |  |
| `sessions_create` | rdzeń, aplikacja, bieg aplikacji |  |
| `sessions_duplicate_as_template` | aplikacja, bieg aplikacji |  |
| `sessions_export` | rdzeń, aplikacja, bieg aplikacji |  |
| `sessions_export_conversation` | rdzeń, aplikacja, bieg aplikacji |  |
| `sessions_get_draft` | rdzeń, aplikacja, bieg aplikacji |  |
| `sessions_list` | rdzeń, aplikacja, bieg aplikacji |  |
| `sessions_mark_read` | aplikacja, bieg aplikacji |  |
| `sessions_remove` | rdzeń, aplikacja, bieg aplikacji |  |
| `sessions_rename` | rdzeń, aplikacja, bieg aplikacji |  |
| `sessions_save_draft` | rdzeń, aplikacja, bieg aplikacji |  |
| `sessions_search` | rdzeń, aplikacja, bieg aplikacji |  |
| `sessions_set_archived` | aplikacja, bieg aplikacji |  |
| `sessions_set_pinned` | aplikacja, bieg aplikacji |  |
| `sessions_set_project` | rdzeń, aplikacja, bieg aplikacji |  |
| `sessions_undo_remove` | rdzeń, aplikacja, bieg aplikacji |  |
| `sessions_workdir` | rdzeń, aplikacja, bieg aplikacji |  |
| `settings_reset` | rdzeń, aplikacja, bieg aplikacji |  |
| `settings_schema` | rdzeń, aplikacja, bieg aplikacji |  |
| `settings_set` | rdzeń, aplikacja, bieg aplikacji |  |
| `settings_set_shortcut` | rdzeń, aplikacja, bieg aplikacji |  |
| `settings_values` | rdzeń, aplikacja, bieg aplikacji |  |
| `skills_approve` | rdzeń, aplikacja, bieg aplikacji |  |
| `skills_disable` | aplikacja, bieg aplikacji |  |
| `skills_export` | rdzeń, aplikacja |  |
| `skills_import` | rdzeń |  |
| `skills_list` | rdzeń, aplikacja, bieg aplikacji |  |
| `skills_propose` | rdzeń, aplikacja, bieg aplikacji |  |
| `skills_reject` | aplikacja, bieg aplikacji |  |
| `skills_release` | aplikacja, bieg aplikacji |  |
| `skills_review` | rdzeń, aplikacja, bieg aplikacji |  |
| `skills_run` | rdzeń, aplikacja, bieg aplikacji |  |
| `system_clipboard_text` | rdzeń, bieg aplikacji |  |
| `system_retry_queue` | rdzeń, aplikacja, bieg aplikacji |  |
| `system_status` | rdzeń, aplikacja, eval, bieg aplikacji |  |
| `tasks_cancel` | rdzeń, aplikacja, bieg aplikacji |  |
| `tasks_create` | rdzeń, aplikacja, bieg aplikacji |  |
| `tasks_list` | rdzeń, aplikacja, bieg aplikacji |  |
| `tasks_pause` | aplikacja, bieg aplikacji |  |
| `tasks_resume` | aplikacja, bieg aplikacji |  |
| `tasks_retry` | rdzeń, aplikacja, bieg aplikacji |  |
| `tasks_steer` | aplikacja, bieg aplikacji |  |
| `terminal_close` | rdzeń, bieg aplikacji |  |
| `terminal_input` | rdzeń, bieg aplikacji |  |
| `terminal_list` | rdzeń, aplikacja, bieg aplikacji |  |
| `terminal_resize` | rdzeń, aplikacja, bieg aplikacji |  |
| `timeline_list` | rdzeń, aplikacja, bieg aplikacji |  |
| `transfer_export` | rdzeń, bieg aplikacji |  |
| `transfer_import` | rdzeń, aplikacja, bieg aplikacji |  |
| `transfer_inspect` | rdzeń, aplikacja, bieg aplikacji |  |
| `transfer_rollback` | rdzeń, aplikacja, bieg aplikacji |  |
| `triggers_create` | rdzeń, aplikacja, bieg aplikacji |  |
| `triggers_fire_now` | rdzeń, aplikacja, bieg aplikacji |  |
| `triggers_list` | rdzeń, aplikacja, bieg aplikacji |  |
| `triggers_log` | rdzeń, aplikacja, bieg aplikacji |  |
| `triggers_preview_cron` | rdzeń, aplikacja, bieg aplikacji |  |
| `triggers_remove` | aplikacja, bieg aplikacji |  |
| `triggers_set_enabled` | aplikacja, bieg aplikacji |  |
| `turns_arena_answer` | rdzeń, aplikacja |  |
| `turns_continue` | rdzeń, aplikacja, bieg aplikacji |  |
| `turns_edit_and_resend` | rdzeń, aplikacja, bieg aplikacji |  |
| `turns_list` | rdzeń, aplikacja, bieg aplikacji |  |
| `turns_rate` | rdzeń, aplikacja, bieg aplikacji |  |
| `turns_read_aloud` | rdzeń, aplikacja, bieg aplikacji |  |
| `turns_regenerate` | rdzeń, aplikacja, bieg aplikacji |  |
| `turns_remember` | rdzeń, aplikacja, bieg aplikacji |  |
| `turns_run_code` | rdzeń, aplikacja, bieg aplikacji |  |
| `turns_save_code` | aplikacja, bieg aplikacji |  |
| `turns_select_branch` | rdzeń, aplikacja |  |
| `turns_send` | rdzeń, bieg aplikacji |  |
| `turns_set_hidden` | rdzeń, aplikacja, bieg aplikacji |  |
| `turns_stop` | rdzeń, aplikacja, bieg aplikacji |  |
| `turns_undo_step` | rdzeń, aplikacja, bieg aplikacji |  |
| `updates_about` | aplikacja, bieg aplikacji |  |
| `updates_cancel` | aplikacja, bieg aplikacji |  |
| `updates_check` | rdzeń, aplikacja |  |
| `updates_dismiss_whats_new` | aplikacja, bieg aplikacji |  |
| `updates_download` | aplikacja, bieg aplikacji |  |
| `updates_history` | aplikacja, bieg aplikacji |  |
| `updates_restart` | aplikacja, bieg aplikacji |  |
| `updates_rollback` | aplikacja, bieg aplikacji |  |
| `updates_status` | rdzeń, aplikacja, bieg aplikacji |  |
| `updates_whats_new` | aplikacja, bieg aplikacji |  |
| `voice_devices` | rdzeń, aplikacja, bieg aplikacji |  |
| `voice_dictation` | aplikacja, bieg aplikacji |  |
| `voice_engines` | aplikacja, bieg aplikacji |  |
| `voice_features` | aplikacja, bieg aplikacji |  |
| `voice_preview` | aplikacja, bieg aplikacji |  |
| `voice_ptt` | interfejs, aplikacja, bieg aplikacji |  |
| `voice_read` | aplikacja, bieg aplikacji |  |
| `voice_set_mic_enabled` | rdzeń, aplikacja, bieg aplikacji |  |
| `voice_set_muted` | rdzeń, aplikacja, bieg aplikacji |  |
| `voice_speaker` | aplikacja, bieg aplikacji |  |
| `voice_start_mic_test` | rdzeń, bieg aplikacji |  |
| `voice_status` | rdzeń, aplikacja, bieg aplikacji |  |
| `voice_stop_mic_test` | rdzeń, aplikacja, bieg aplikacji |  |
| `voice_stop_speech` | rdzeń, aplikacja, bieg aplikacji |  |
| `voice_wake` | aplikacja, bieg aplikacji |  |

## Polecenia palety i skróty

| Funkcja | Testy | Uwagi |
| --- | --- | --- |
| `session.new` | przegląd ogólny |  |
| `session.switch` | przegląd ogólny |  |
| `session.close` | przegląd ogólny |  |
| `session.reopen` | przegląd ogólny |  |
| `session.next` | przegląd ogólny |  |
| `session.prev` | przegląd ogólny |  |
| `session.goto1` | przegląd ogólny |  |
| `session.rename` | przegląd ogólny |  |
| `search.everywhere` | przegląd ogólny |  |
| `search.conversation` | przegląd ogólny |  |
| `palette.open` | rdzeń, interfejs, przegląd ogólny |  |
| `cheatsheet.open` | przegląd ogólny |  |
| `help.open` | przegląd ogólny |  |
| `settings.open` | przegląd ogólny |  |
| `view.focus` | przegląd ogólny |  |
| `view.zoomIn` | interfejs, przegląd ogólny |  |
| `view.zoomOut` | przegląd ogólny |  |
| `view.zoomReset` | interfejs, przegląd ogólny |  |
| `nav.back` | przegląd ogólny |  |
| `nav.forward` | przegląd ogólny |  |
| `panel.sessions` | interfejs, przegląd ogólny |  |
| `panel.right` | interfejs, przegląd ogólny |  |
| `panel.agents` | przegląd ogólny |  |
| `panel.timeline` | przegląd ogólny |  |
| `panel.files` | przegląd ogólny |  |
| `panel.memory` | przegląd ogólny |  |
| `panel.screen` | przegląd ogólny |  |
| `panel.voice` | przegląd ogólny |  |
| `panel.tasks` | przegląd ogólny |  |
| `voice.mic` | rdzeń, przegląd ogólny |  |
| `voice.ptt` | interfejs, przeglądarka | klawisz pola pisania |
| `conversation.escape` | przeglądarka | klawisz pola pisania |
| `composer.send` | przeglądarka | klawisz pola pisania |
| `composer.newline` | przeglądarka | klawisz pola pisania |
| `composer.editLast` | przeglądarka | klawisz pola pisania |
| `composer.historyPrev` | przeglądarka | klawisz pola pisania |
| `composer.historyNext` | przeglądarka | klawisz pola pisania |
| `composer.pastePlain` | przeglądarka | klawisz pola pisania |
| `system.quickAsk` | interfejs | skrót globalny |
| `system.killSwitch` | aplikacja | skrót globalny |
| `voice.dictation` | aplikacja | skrót globalny |
| `voice.readSelection` | aplikacja | skrót globalny |
| `action.theme` | przegląd ogólny |  |
| `action.language` | przeglądarka, przegląd ogólny |  |
| `action.column` | przegląd ogólny |  |
| `action.panels` | przegląd ogólny |  |
| `action.transfer` | przegląd ogólny |  |
| `action.accounts` | przegląd ogólny |  |
| `action.autonomy` | przegląd ogólny |  |
| `action.cast` | przegląd ogólny |  |
| `action.stopGeneration` | przegląd ogólny |  |
| `action.onboarding` | przegląd ogólny |  |
| `action.exportMarkdown` | przegląd ogólny |  |
| `action.exportHtml` | przegląd ogólny |  |
| `action.exportJson` | przegląd ogólny |  |
| `action.templates` | przegląd ogólny |  |
| `action.help` | przegląd ogólny |  |
| `action.board` | przegląd ogólny |  |
| `action.stats` | przegląd ogólny |  |
| `action.arena` | przegląd ogólny |  |
| `action.copyConversation` | przegląd ogólny |  |
| `action.teach` | przegląd ogólny |  |

## Strony Ustawień

| Funkcja | Testy | Uwagi |
| --- | --- | --- |
| `general` | przeglądarka, przegląd ogólny | Ogólne |
| `connections` | przeglądarka, przegląd ogólny | Stan połączeń |
| `providers` | przeglądarka, przegląd ogólny | Konta i klucze API |
| `api` | przeglądarka, przegląd ogólny | Kolejki modeli API |
| `bridges` | przeglądarka, aplikacja, przegląd ogólny | Mosty CLI |
| `models` | przeglądarka, aplikacja, przegląd ogólny | Modele lokalne i silniki |
| `costs` | przeglądarka, przegląd ogólny | Koszty i limity |
| `router` | przeglądarka, aplikacja, przegląd ogólny | Dobór modelu |
| `voice` | interfejs, przeglądarka, aplikacja, przegląd ogólny | Głos |
| `agents` | przeglądarka, przegląd ogólny | Agentki |
| `skills` | przeglądarka, przegląd ogólny | Umiejętności |
| `plugins` | przeglądarka, przegląd ogólny | Wtyczki |
| `mcp` | przeglądarka, przegląd ogólny | Serwery MCP |
| `diag` | przeglądarka, przegląd ogólny | Diagnostyka komputera |
| `builder` | przeglądarka, przegląd ogólny | Kreator agentek |
| `triggers` | przeglądarka, przegląd ogólny | Zadania w tle i wyzwalacze |
| `marshal` | przeglądarka, przegląd ogólny | Reguły Marszałka |
| `permissions` | przeglądarka, aplikacja, przegląd ogólny | Uprawnienia i bezpieczeństwo |
| `computer` | aplikacja, przegląd ogólny | Komputer |
| `memory` | interfejs, przeglądarka, przegląd ogólny | Pamięć |
| `sessions` | przegląd ogólny | Sesje i okna |
| `files` | interfejs, przeglądarka, przegląd ogólny | Pliki |
| `logs` | przegląd ogólny | Logi i prywatność; wkrótce |
| `improve` | przeglądarka, przegląd ogólny | Zdrowie systemu |
| `modules` | przeglądarka, przegląd ogólny | Moduły |
| `cameras` | przeglądarka, aplikacja, przegląd ogólny | Kamery |
| `devices` | przeglądarka, aplikacja, przegląd ogólny | Urządzenia |
| `transfer` | przeglądarka, aplikacja, przegląd ogólny | Import i eksport |
| `appearance` | przeglądarka, aplikacja, przegląd ogólny | Wygląd |
| `shortcuts` | przeglądarka, przegląd ogólny | Skróty |
| `commands` | przeglądarka, przegląd ogólny | Polecenia bez modelu |
| `templates` | przeglądarka, przegląd ogólny | Szablony promptów |
| `notifications` | przegląd ogólny | Powiadomienia |
| `language` | przeglądarka, przegląd ogólny | Język |
| `updates` | przeglądarka, aplikacja, przegląd ogólny | Aktualizacje |
| `advanced` | przeglądarka, aplikacja, przegląd ogólny | Zaawansowane |
| `about` | przeglądarka, przegląd ogólny | O programie |
| `help` | przeglądarka, przegląd ogólny | Instrukcja |

## Ustawienia

| Funkcja | Testy | Uwagi |
| --- | --- | --- |
| `general.start_with_system` | aplikacja, przegląd ogólny | Uruchamiaj z systemem |
| `general.close_to_tray` | przegląd ogólny | Zamknięcie okna chowa do zasobnika |
| `general.destroy_webview_after` | rdzeń, przegląd ogólny | Zwolnij pamięć okna po |
| `general.view_profile` | przegląd ogólny | Profil widoku ustawień |
| `quick.session_mode` | przegląd ogólny | Szybkie pytanie: sesja |
| `chat.reply_style` | rdzeń, przegląd ogólny | Styl odpowiedzi |
| `chat.temperature` | rdzeń, przegląd ogólny | Swoboda odpowiedzi |
| `models.default_profile` | przegląd ogólny | Domyślny profil modelu |
| `models.effort` | przegląd ogólny | Wysiłek rozumowania |
| `accounts.check_interval_hours` | rdzeń, przegląd ogólny | Sprawdzanie kluczy w tle |
| `accounts.key_max_age_days` | przegląd ogólny | Przypomnienie o wymianie klucza |
| `engines.llm.backend` | przegląd ogólny | Silnik modelu rozmowy |
| `engines.llm.context` | przegląd ogólny | Długość kontekstu |
| `engines.llm.threads` | przegląd ogólny | Wątki procesora |
| `engines.llm.idle_unload_min` | przegląd ogólny | Zwolnij model po bezczynności |
| `engines.llm.tool_schema` | przegląd ogólny | Poprawne wywołania narzędzi (model lokalny) |
| `models.parallel_downloads` | przegląd ogólny | Pobierania naraz |
| `costs.warn_at` | przegląd ogólny | Ostrzegaj przy zużyciu limitu |
| `costs.background_local_only` | przegląd ogólny | Zadania w tle tylko lokalnie |
| `costs.estimate_before_long` | przegląd ogólny | Szacuj koszt przed długim zadaniem |
| `voice.noise_reduction` | przegląd ogólny | Redukcja szumów mikrofonu |
| `voice.tts_style` | przegląd ogólny | Styl mówienia |
| `voice.tts_speed` | interfejs, przeglądarka, aplikacja, przegląd ogólny | Tempo mowy |
| `voice.tts_volume` | przegląd ogólny | Głośność mowy |
| `voice.tts_pause` | przegląd ogólny | Pauza między zdaniami |
| `voice.tts_character` | przegląd ogólny | Różnice między głosami agentek |
| `voice.tts_voice_alfa` | przegląd ogólny | Głos Alfy |
| `voice.tts_voice_beta` | przegląd ogólny | Głos Bety |
| `voice.tts_voice_gama` | przegląd ogólny | Głos Gamy |
| `voice.tts_voice_delta` | przegląd ogólny | Głos Delty |
| `voice.talk_smart_turn` | przegląd ogólny | Wyczucie końca wypowiedzi (Smart Turn) |
| `voice.talk_turn_threshold` | przegląd ogólny | Pewność końca wypowiedzi |
| `voice.talk_max_silence` | przegląd ogólny | Najdłuższe czekanie w ciszy |
| `voice.talk_stt_engine` | przegląd ogólny | Silnik rozpoznawania mowy |
| `voice.talk_language` | przegląd ogólny | Język rozpoznawania mowy |
| `voice.mode` | interfejs, przegląd ogólny | Włączanie mikrofonu |
| `voice.show_pill` | przegląd ogólny | Pigułka głosowa |
| `agents.max_steps` | rdzeń, interfejs, przegląd ogólny | Limit kroków zadania |
| `agents.auto_extend_steps` | rdzeń, przeglądarka, przegląd ogólny | Zgoda na automatyczne przedłużanie kroków |
| `agents.max_minutes` | przegląd ogólny | Limit czasu zadania |
| `agents.auto_extend_time` | przeglądarka, przegląd ogólny | Zgoda na automatyczne przedłużanie czasu |
| `agents.max_cost_pln` | przegląd ogólny | Limit kosztu zadania |
| `agents.approval_timeout_s` | przegląd ogólny | Czekanie na zatwierdzenie |
| `agents.auto_extend_approval` | przeglądarka, przegląd ogólny | Automatycznie przedłużaj oczekiwanie na zatwierdzenie |
| `agents.verify_before_done` | rdzeń, przegląd ogólny | „Gotowe" po samoweryfikacji |
| `scheduler.max_parallel` | przegląd ogólny | Zadania naraz |
| `agents.alfa.access` | przegląd ogólny | Alfa — Profil dostępu |
| `agents.alfa.groups` | przegląd ogólny | Alfa — Grupy narzędzi |
| `agents.alfa.read_only` | przegląd ogólny | Alfa — Własny profil: tylko odczyt |
| `agents.beta.access` | przegląd ogólny | Beta — Profil dostępu |
| `agents.beta.groups` | przegląd ogólny | Beta — Grupy narzędzi |
| `agents.beta.read_only` | przegląd ogólny | Beta — Własny profil: tylko odczyt |
| `agents.gama.access` | przegląd ogólny | Gama — Profil dostępu |
| `agents.gama.groups` | przegląd ogólny | Gama — Grupy narzędzi |
| `agents.gama.read_only` | przegląd ogólny | Gama — Własny profil: tylko odczyt |
| `agents.delta.access` | rdzeń, przegląd ogólny | Delta — Profil dostępu |
| `agents.delta.groups` | rdzeń, przegląd ogólny | Delta — Grupy narzędzi |
| `agents.delta.read_only` | przegląd ogólny | Delta — Własny profil: tylko odczyt |
| `ui.hide_from_capture` | przegląd ogólny | Ukrywaj okna Alfy na zrzutach i nagraniach ekranu |
| `ui.broker_dev_banner` | przegląd ogólny | Pokazuj baner trybu deweloperskiego |
| `memory.consolidation_enabled` | przegląd ogólny | Porządkowanie nocne |
| `memory.consolidation_window` | przegląd ogólny | Okno porządkowania |
| `memory.auto_extract` | przegląd ogólny | Fakty z rozmów |
| `sessions.default_template` | przegląd ogólny | Szablon nowej sesji |
| `sessions.auto_title` | przegląd ogólny | Automatyczny tytuł |
| `sessions.delete_undo_seconds` | przegląd ogólny | Czas na cofnięcie usunięcia |
| `ui.max_detached_windows` | przegląd ogólny | Limit odłączonych okien |
| `files.workdir_root` | przegląd ogólny | Katalog wyjściowy |
| `files.image_thumbnails` | przegląd ogólny | Miniatury obrazów |
| `diagnostician.autonomy` | rdzeń, eval, przegląd ogólny | Samodzielność Diagnosty |
| `camera.enabled` | rdzeń, aplikacja, przegląd ogólny | Kamery włączone |
| `camera.auto_look` | przegląd ogólny | „Opisz, co widzisz” robi zdjęcie |
| `device.battery_mode` | przegląd ogólny | Tryb baterii |
| `device.game_mode` | przegląd ogólny | Tryb gry / pełnego ekranu |
| `ui.theme` | rdzeń, interfejs, przeglądarka, aplikacja, eval, przegląd ogólny | Motyw |
| `ui.style` | interfejs, aplikacja, przegląd ogólny | Styl |
| `ui.accent` | przegląd ogólny | Akcent |
| `ui.density` | przegląd ogólny | Gęstość |
| `ui.zoom` | interfejs, przegląd ogólny | Powiększenie |
| `ui.zoom_fit` | interfejs, przegląd ogólny | Dopasuj skalę do okna |
| `ui.animations` | przegląd ogólny | Animacje |
| `ui.column` | interfejs, przegląd ogólny | Szerokość kolumny rozmowy |
| `ui.panels_autohide` | aplikacja, przegląd ogólny | Panele chowają się same |
| `ui.simple` | interfejs, przeglądarka, aplikacja, przegląd ogólny | Uproszczony ekran |
| `ui.avatars` | przegląd ogólny | Awatary agentek |
| `ui.reply_details` | przegląd ogólny | Szczegóły pod odpowiedzią |
| `composer.enter_sends` | przegląd ogólny | Enter wysyła wiadomość |
| `sessions.local_commands` | rdzeń, przegląd ogólny | Odpowiadaj bez modelu |
| `notify.toasts` | przegląd ogólny | Powiadomienia Windows |
| `notify.earcons` | interfejs, aplikacja, przegląd ogólny | Dźwięki sygnałowe |
| `notify.earcon_volume` | przegląd ogólny | Głośność dźwięków |
| `notify.dnd_fullscreen` | przegląd ogólny | Nie przeszkadzać na pełnym ekranie |
| `ui.locale` | interfejs, przeglądarka, przegląd ogólny | Język interfejsu |
| `updates.channel` | interfejs, przegląd ogólny | Kanał aktualizacji |
| `updates.mode` | przegląd ogólny | Instalowanie aktualizacji |
| `updates.whats_new` | przegląd ogólny | Pokaż „Co nowego" |
| `engine.nextgo.root` | rdzeń, aplikacja, przegląd ogólny | Katalog silnika gier NextGO.Gray |
| `experimental.diag_daily` | przegląd ogólny | Codzienny zapis parametrów komputera |

## Narzędzia agentek

| Funkcja | Testy | Uwagi |
| --- | --- | --- |
| `browser_click` | rdzeń, eval, przegląd ogólny |  |
| `browser_close` | rdzeń, eval, przegląd ogólny |  |
| `browser_console` | rdzeń, przegląd ogólny |  |
| `browser_key` | rdzeń, przegląd ogólny |  |
| `browser_nav` | rdzeń, przegląd ogólny |  |
| `browser_open` | rdzeń, eval, przegląd ogólny |  |
| `browser_read` | rdzeń, eval, przegląd ogólny |  |
| `browser_screenshot` | rdzeń, eval, przegląd ogólny |  |
| `browser_scroll` | rdzeń, przegląd ogólny |  |
| `browser_select` | rdzeń, przegląd ogólny |  |
| `browser_type` | rdzeń, eval, przegląd ogólny |  |
| `browser_wait` | rdzeń, przegląd ogólny |  |
| `clipboard_read` | rdzeń, eval, przegląd ogólny |  |
| `clipboard_write` | rdzeń, eval, przegląd ogólny |  |
| `dev_git` | rdzeń, przegląd ogólny |  |
| `dev_test` | rdzeń, przegląd ogólny |  |
| `engine_capture` | rdzeń, przegląd ogólny |  |
| `engine_docs` | rdzeń, przegląd ogólny |  |
| `engine_height` | rdzeń, przegląd ogólny |  |
| `engine_inspect` | rdzeń, przegląd ogólny |  |
| `engine_preview` | rdzeń, przegląd ogólny |  |
| `engine_raycast` | rdzeń, przegląd ogólny |  |
| `engine_rollback` | rdzeń, przegląd ogólny |  |
| `engine_select` | rdzeń, przegląd ogólny |  |
| `engine_shader` | rdzeń, przegląd ogólny |  |
| `engine_start` | rdzeń, przegląd ogólny |  |
| `engine_status` | rdzeń, przegląd ogólny |  |
| `engine_step` | rdzeń, przegląd ogólny |  |
| `engine_stop` | rdzeń, przegląd ogólny |  |
| `engine_stream` | rdzeń, przegląd ogólny |  |
| `engine_transaction` | rdzeń, przegląd ogólny |  |
| `fs_copy` | rdzeń, eval, przegląd ogólny |  |
| `fs_delete` | rdzeń, przeglądarka, eval, przegląd ogólny |  |
| `fs_delete_permanent` | rdzeń, eval, przegląd ogólny |  |
| `fs_list` | rdzeń, eval, przegląd ogólny |  |
| `fs_mkdir` | rdzeń, eval, przegląd ogólny |  |
| `fs_move` | rdzeń, eval, przegląd ogólny |  |
| `fs_read` | rdzeń, eval, przegląd ogólny |  |
| `fs_rename` | rdzeń, eval, przegląd ogólny |  |
| `fs_search` | rdzeń, eval, przegląd ogólny |  |
| `fs_stat` | rdzeń, eval, przegląd ogólny |  |
| `fs_write` | rdzeń, interfejs, aplikacja, eval, przegląd ogólny |  |
| `input_click` | rdzeń, eval, przegląd ogólny |  |
| `input_keys` | rdzeń, eval, przegląd ogólny |  |
| `input_scroll` | rdzeń, eval, przegląd ogólny |  |
| `input_type_text` | rdzeń, eval, przegląd ogólny |  |
| `mcp_call` | rdzeń, przegląd ogólny |  |
| `mcp_tools` | rdzeń, aplikacja, przegląd ogólny |  |
| `media_convert` | rdzeń, eval, przegląd ogólny |  |
| `media_info` | rdzeń, eval, przegląd ogólny |  |
| `media_play` | rdzeń, eval, przegląd ogólny |  |
| `memory_recall` | rdzeń, przegląd ogólny |  |
| `memory_remember` | rdzeń, przegląd ogólny |  |
| `net_download` | rdzeń, eval, przegląd ogólny |  |
| `net_fetch` | rdzeń, eval, przegląd ogólny |  |
| `office_edit` | rdzeń, eval, przegląd ogólny |  |
| `office_read` | rdzeń, eval, przegląd ogólny |  |
| `screen_capture` | rdzeń, eval, przegląd ogólny |  |
| `shell_run` | rdzeń, eval, przegląd ogólny |  |
| `shell_terminal` | rdzeń, eval, przegląd ogólny |  |
| `system_diagnostics` | rdzeń, przegląd ogólny |  |
| `system_env` | rdzeń, eval, przegląd ogólny |  |
| `system_env_set` | rdzeń, eval, przegląd ogólny |  |
| `system_events` | rdzeń, eval, przegląd ogólny |  |
| `system_process_info` | rdzeń, eval, przegląd ogólny |  |
| `system_process_kill` | rdzeń, eval, przegląd ogólny |  |
| `system_processes` | rdzeń, eval, przegląd ogólny |  |
| `system_service_control` | rdzeń, eval, przegląd ogólny |  |
| `system_services` | rdzeń, eval, przegląd ogólny |  |
| `system_software` | rdzeń, przegląd ogólny |  |
| `system_status` | rdzeń, aplikacja, eval, przegląd ogólny |  |
| `uia_act` | rdzeń, eval, przegląd ogólny |  |
| `uia_find` | rdzeń, eval, przegląd ogólny |  |
| `uia_read_text` | rdzeń, eval, przegląd ogólny |  |
| `uia_tree` | rdzeń, eval, przegląd ogólny |  |
| `vision_describe` | rdzeń, eval, przegląd ogólny |  |
| `vision_ocr` | rdzeń, eval, przegląd ogólny |  |
| `window_focus` | rdzeń, eval, przegląd ogólny |  |
| `window_list` | rdzeń, eval, przegląd ogólny |  |
| `window_move` | rdzeń, eval, przegląd ogólny |  |
| `window_state` | rdzeń, eval, przegląd ogólny |  |

## Zdarzenia rdzenia

| Funkcja | Testy | Uwagi |
| --- | --- | --- |
| `TurnAppended` | rdzeń, przeglądarka, przegląd ogólny |  |
| `TurnStatus` | rdzeń, interfejs, przegląd ogólny |  |
| `TextDelta` | rdzeń, interfejs, przeglądarka, przegląd ogólny |  |
| `ThinkingDelta` | rdzeń, przegląd ogólny |  |
| `ToolCall` | rdzeń, przegląd ogólny |  |
| `ApprovalPending` | rdzeń, przegląd ogólny |  |
| `Usage` | rdzeń, interfejs, przegląd ogólny |  |
| `Stop` | rdzeń, interfejs, przeglądarka, eval, przegląd ogólny |  |
| `Error` | rdzeń, interfejs, przeglądarka, aplikacja, przegląd ogólny |  |
| `SessionUpdated` | rdzeń, przegląd ogólny |  |
| `SessionRemoved` | rdzeń, interfejs, przegląd ogólny |  |
| `AgentsChanged` | interfejs, przegląd ogólny |  |
| `ActivityChanged` | rdzeń, przegląd ogólny |  |
| `CostsChanged` | interfejs, przegląd ogólny |  |
| `SystemStatusChanged` | interfejs, przegląd ogólny |  |
| `TimelineAppended` | rdzeń, przegląd ogólny |  |
| `AccountChanged` | rdzeń, przegląd ogólny |  |
| `MicLevel` | rdzeń, przegląd ogólny |  |
| `SpeechLevel` | rdzeń, przegląd ogólny |  |
| `SpeechSegment` | rdzeń, przeglądarka, przegląd ogólny |  |
| `VoicePill` | rdzeń, przegląd ogólny |  |
| `Toast` | rdzeń, przegląd ogólny |  |
| `OpenSession` | rdzeń, interfejs, przegląd ogólny |  |
| `AgentRunUpdated` | rdzeń, interfejs, przegląd ogólny |  |
| `AgentStep` | rdzeń, interfejs, przegląd ogólny |  |
| `VoiceStatusChanged` | rdzeń, przegląd ogólny |  |
| `LocalModelProgress` | interfejs, przegląd ogólny |  |
| `MemoryChanged` | rdzeń, interfejs, przegląd ogólny |  |
| `TaskUpdated` | rdzeń, interfejs, przegląd ogólny |  |
| `TriggerFired` | rdzeń, przegląd ogólny |  |
| `MarshalReportReady` | interfejs, przegląd ogólny |  |
| `GuiActivity` | rdzeń, interfejs, przegląd ogólny |  |
| `SkillsChanged` | rdzeń, interfejs, przegląd ogólny |  |
| `HealthChanged` | rdzeń, interfejs, przegląd ogólny |  |
| `UpdateStatus` | rdzeń, interfejs, przegląd ogólny |  |
| `BrokerStatus` | interfejs, przegląd ogólny |  |
| `VoiceFeaturesChanged` | rdzeń, interfejs, przegląd ogólny |  |
| `ModelProgress` | rdzeń, interfejs, przegląd ogólny |  |
| `ModelChanged` | rdzeń, interfejs, przegląd ogólny |  |
| `ReindexStatus` | rdzeń, interfejs, przegląd ogólny |  |
| `CamerasChanged` | rdzeń, przegląd ogólny |  |

