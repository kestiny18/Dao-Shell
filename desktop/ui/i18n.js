/* Only explicit built-in message keys are translated. User data is never scanned. */
const messages = {
  "app.ready": {
    "zh": "准备就绪",
    "en": "Ready"
  },
  "app.workspace_was_not_restored_and_cannot_be_saved_close_the_app_bac": {
    "zh": "工作区未恢复，不能保存。请关闭应用，备份并移走配置目录中的 workspace.json 后重启。",
    "en": "Workspace was not restored and cannot be saved. Close the app, back up and move workspace.json out of the configuration directory, then restart."
  },
  "app.not_saved_value_check_disk_space_and_configuration_folder_permis": {
    "zh": "尚未保存：{v0}。请检查磁盘和配置目录权限，然后点击“重试保存”。",
    "en": "Not saved: {v0}. Check disk space and configuration folder permissions, then choose Retry save."
  },
  "app.session_name": {
    "zh": "会话名称",
    "en": "Session name"
  },
  "app.save_name": {
    "zh": "保存名称",
    "en": "Save name"
  },
  "app.cancel_rename": {
    "zh": "取消重命名",
    "en": "Cancel rename"
  },
  "app.name_is_too_long_use_at_most_512_utf_8_bytes": {
    "zh": "名称过长，请控制在 512 字节以内（约 170 个汉字）。",
    "en": "Name is too long. Use at most 512 UTF-8 bytes."
  },
  "app.enter_a_session_name": {
    "zh": "请输入会话名称。",
    "en": "Enter a session name."
  },
  "app.saving": {
    "zh": "正在保存…",
    "en": "Saving…"
  },
  "app.name_was_not_saved_value_please_retry": {
    "zh": "名称未保存：{v0}。请重试。",
    "en": "Name was not saved: {v0}. Please retry."
  },
  "app.open_session_value": {
    "zh": "打开会话：{v0}",
    "en": "Open session: {v0}"
  },
  "app.rename_session_value": {
    "zh": "重命名会话：{v0}",
    "en": "Rename session: {v0}"
  },
  "app.overview": {
    "zh": "概览",
    "en": "Overview"
  },
  "app.settings": {
    "zh": "设置",
    "en": "Settings"
  },
  "app.session": {
    "zh": "会话",
    "en": "Session"
  },
  "app.switch_to_value": {
    "zh": "切换到{v0}",
    "en": "Switch to {v0}"
  },
  "app.close_tab_value": {
    "zh": "关闭标签：{v0}",
    "en": "Close tab: {v0}"
  },
  "app.operation_incomplete_details": {
    "zh": "操作未完成 · 查看详情",
    "en": "Operation incomplete · Details"
  },
  "app.some_indexes_are_unavailable_details": {
    "zh": "部分索引当前不可用 · 查看详情",
    "en": "Some indexes are unavailable · Details"
  },
  "app.search_coverage_is_incomplete_details": {
    "zh": "搜索覆盖不完整 · 查看详情",
    "en": "Search coverage is incomplete · Details"
  },
  "app.search_results_history": {
    "zh": "搜索结果（历史记录",
    "en": "Search results (history"
  },
  "app.search_record_value_items_details": {
    "zh": "搜索记录（{v0} 项）· 查看详情",
    "en": "Search record · Items: {v0} · Details"
  },
  "app.open_request_handed_to_windows_details": {
    "zh": "打开请求已交给系统 · 查看详情",
    "en": "Open request handed to Windows · Details"
  },
  "app.local_search_and_action_record_details": {
    "zh": "本地搜索与操作记录 · 查看详情",
    "en": "Local search and action record · Details"
  },
  "app.you": {
    "zh": "你",
    "en": "You"
  },
  "app.value_items": {
    "zh": "{v0} 项",
    "en": "Items: {v0}"
  },
  "app.open": {
    "zh": "打开 ↗",
    "en": "Open ↗"
  },
  "app.open_value": {
    "zh": "打开 {v0}",
    "en": "Open {v0}"
  },
  "app.another_session_is_running_you_can_switch_to_it_or_stop_it": {
    "zh": "另一会话正在处理，可切换查看或停止。",
    "en": "Another session is running. You can switch to it or stop it."
  },
  "app.skipped_value_items": {
    "zh": " 跳过 {v0} 项。",
    "en": " Skipped items: {v0}."
  },
  "app.index_coverage_is_incomplete": {
    "zh": " 索引覆盖不完整。",
    "en": " Index coverage is incomplete."
  },
  "app.scan_was_truncated_results_are_incomplete": {
    "zh": " 本次扫描已截断，结果不完整。",
    "en": " Scan was truncated; results are incomplete."
  },
  "app.search_results_history_search_again_before_acting": {
    "zh": "搜索结果（历史记录；操作前请重新搜索）：\n",
    "en": "Search results (history; search again before acting):\n"
  },
  "app.open_request_processed": {
    "zh": "打开请求已处理。",
    "en": "Open request processed."
  },
  "app.waiting_for_your_confirmation": {
    "zh": "等待你的确认…",
    "en": "Waiting for your confirmation…"
  },
  "app.new_session": {
    "zh": "新会话",
    "en": "New session"
  },
  "app.explain_this_computer_resource_snapshot": {
    "zh": "解释这次电脑资源快照",
    "en": "Explain this computer resource snapshot"
  },
  "app.working": {
    "zh": "正在处理…",
    "en": "Working…"
  },
  "app.stopped": {
    "zh": "已停止",
    "en": "Stopped"
  },
  "app.this_request_did_not_complete_adjust_it_and_try_again": {
    "zh": "这次没有完成，可以调整后再试。",
    "en": "This request did not complete. Adjust it and try again."
  },
  "app.request_incomplete": {
    "zh": "请求未完成",
    "en": "Request incomplete"
  },
  "app.stop_the_current_request_and_close_this_tab": {
    "zh": "停止当前请求并关闭标签？",
    "en": "Stop the current request and close this tab?"
  },
  "app.restore_failed_value_the_original_file_is_preserved_this_session": {
    "zh": "恢复失败：{v0}。原文件保留；本次内容不会保存。请关闭应用，备份并移走配置目录中的 workspace.json 后重启。",
    "en": "Restore failed: {v0}. The original file is preserved; this session will not be saved. Close the app, back up and move workspace.json out of the configuration directory, then restart."
  },
  "app.ui_preview_file_and_model_features_are_available_only_in_the_des": {
    "zh": "界面预览：文件与模型能力仅在桌面应用中可用。",
    "en": "UI preview: file and model features are available only in the desktop app."
  },
  "app.static_ui_preview": {
    "zh": "静态界面预览",
    "en": "Static UI preview"
  },
  "app.no_model_connected": {
    "zh": "尚未连接模型",
    "en": "No model connected"
  },
  "app.no_search_folders_yet_choose_folders_in_settings_access": {
    "zh": "还没有查找目录。请在设置 → 访问权限中选择目录。",
    "en": "No search folders yet. Choose folders in Settings → Access."
  },
  "app.configuration_could_not_be_loaded_correct_it_and_restart_the_app": {
    "zh": "配置未能加载，请修正后重启入口。",
    "en": "Configuration could not be loaded. Correct it and restart the app."
  },
  "app.workspace_restore_failed_this_session_was_not_saved_exit_anyway": {
    "zh": "工作区恢复失败，本次内容未保存。仍要退出？",
    "en": "Workspace restore failed; this session was not saved. Exit anyway?"
  },
  "app.save_failed_changes_may_be_lost_exit_anyway": {
    "zh": "保存失败，本次更改可能丢失。仍要退出？",
    "en": "Save failed; changes may be lost. Exit anyway?"
  },
  "app.could_not_listen_for_window_close_events_value": {
    "zh": "无法监听退出事件：{v0}",
    "en": "Could not listen for window close events: {v0}"
  },
  "settings.the_official_value_endpoint_is_filled_in_select_a_model_and_ente": {
    "zh": "{v0} 官方地址已填好。选择模型并填入该服务的 API Key；实际可用模型取决于你的账号。",
    "en": "The official {v0} endpoint is filled in. Select a model and enter this provider's API key. Model availability depends on your account."
  },
  "settings.connect_another_provider_or_local_model_enter_the_endpoint_and_m": {
    "zh": "连接其他服务或本机模型：填写服务地址和模型名称。本机免密服务可不填 API Key。",
    "en": "Connect another provider or local model: enter the endpoint and model ID. Local keyless services do not require an API key."
  },
  "settings.other_model": {
    "zh": "其他模型",
    "en": "Other model"
  },
  "settings.do_not_use_a_model": {
    "zh": "暂不使用模型",
    "en": "Do not use a model"
  },
  "settings.unnamed_model": {
    "zh": "未命名模型",
    "en": "Unnamed model"
  },
  "settings.credentials_available_leave_blank_to_keep": {
    "zh": "已有可用凭据 · 留空保留",
    "en": "Credentials available · Leave blank to keep"
  },
  "settings.no_api_key_set_optional_for_keyless_local_models": {
    "zh": "尚未设置 API Key · 本机免密模型可留空",
    "en": "No API key set · Optional for keyless local models"
  },
  "settings.remove": {
    "zh": "移除",
    "en": "Remove"
  },
  "settings.remove_folder_value": {
    "zh": "移除目录：{v0}",
    "en": "Remove folder: {v0}"
  },
  "settings.no_folders_added": {
    "zh": "尚未添加目录",
    "en": "No folders added"
  },
  "settings.folder_added_to_draft_save_to_apply": {
    "zh": "目录已添加，保存后生效。",
    "en": "Folder added to draft. Save to apply."
  },
  "settings.could_not_choose_a_folder_value": {
    "zh": "无法选择目录：{v0}",
    "en": "Could not choose a folder: {v0}"
  },
  "settings.read_or_open_local_files_available_to_the_current_os_user_these_": {
    "zh": "可读取或打开当前系统用户能访问的本机文件。下面目录用于服务不可用或复杂筛选时的兼容搜索；留空使用下载、文档和桌面。全盘名称搜索默认开启；索引组件可用时，名称发现按系统权限覆盖各卷，写入仍需确认。",
    "en": "Read or open local files available to the current OS user. These folders supplement complex filters or service outages; leave blank for Downloads, Documents and Desktop. Whole-volume name search is enabled by default when its component is available. Writes still require confirmation."
  },
  "settings.these_folders_restrict_reading_opening_and_compatible_folder_sea": {
    "zh": "下面目录限制正文读取、打开及兼容目录搜索；可写目录也在读取范围内，写入仍受可写目录与确认约束。仅搜索下面已授权的目录，不使用全盘名称服务。",
    "en": "These folders restrict reading, opening and compatible folder search. Writable folders are also readable; writes remain subject to folder permissions and confirmation. Searches use only these authorized folders, without the whole-volume service."
  },
  "settings.model_configuration": {
    "zh": "模型配置",
    "en": "Model configuration"
  },
  "settings.enter_the_model_details_test_then_save_choose_the_default_model_": {
    "zh": "填写模型信息，测试后保存；默认模型可在上方选择。",
    "en": "Enter the model details, test, then save. Choose the default model above."
  },
  "settings.the_model_will_be_removed_when_saved_stored_os_credentials_will_": {
    "zh": "模型将在保存后移除；已保存的系统凭据不会被删除。",
    "en": "The model will be removed when saved. Stored OS credentials will be preserved."
  },
  "settings.provider_changed_enter_its_model_details_then_save_to_apply": {
    "zh": "提供方已切换，请填写对应模型信息。保存后生效。",
    "en": "Provider changed. Enter its model details, then save to apply."
  },
  "settings.enter_a_model_id_first": {
    "zh": "请先填写模型名称。",
    "en": "Enter a model ID first."
  },
  "settings.checking_connection_and_tool_calls": {
    "zh": "正在验证连接与工具调用…",
    "en": "Checking connection and tool calls…"
  },
  "settings.connection_and_tool_calls_passed_in_value_ms_not_saved_yet_test_": {
    "zh": "连接与工具调用通过，{v0} ms。尚未保存；修改任何连接信息后需要重新测试。",
    "en": "Connection and tool calls passed in {v0} ms. Not saved yet; test again after changing any connection details."
  },
  "settings.settings_saved_history_is_preserved_model_context_and_old_candid": {
    "zh": "设置已保存。历史已保留，模型上下文和旧候选已失效。",
    "en": "Settings saved. History is preserved; model context and old candidates have been cleared."
  },
  "settings.full_access_saved_whole_volume_search_needs_an_available_matchin": {
    "zh": "完全访问已保存。全盘搜索需要可用且版本匹配的索引组件；未完成组件操作时仅搜索兼容目录，可在文件搜索中管理组件。",
    "en": "Full access saved. Whole-volume search needs an available, matching index component. Until then, searches use compatible folders only. Manage the component under File search."
  },
  "settings.recording": {
    "zh": "记录中",
    "en": "Recording"
  },
  "settings.available": {
    "zh": "可用",
    "en": "Available"
  },
  "settings.update_pending": {
    "zh": "待更新",
    "en": "Update pending"
  },
  "settings.cache_damaged": {
    "zh": "缓存损坏",
    "en": "Cache damaged"
  },
  "settings.folder_unavailable": {
    "zh": "目录不可访问",
    "en": "Folder unavailable"
  },
  "settings.value_value_value_entriesvalue_value": {
    "zh": "{v0} — {v1} · {v2} 条{v3}。{v4}",
    "en": "{v0} — {v1} · Entries: {v2}{v3}. {v4}"
  },
  "settings.update_now": {
    "zh": "立即更新",
    "en": "Update now"
  },
  "settings.stop_this_update": {
    "zh": "停止本次更新",
    "en": "Stop this update"
  },
  "settings.no_accelerated_folders_added": {
    "zh": "尚未添加加速目录。",
    "en": "No accelerated folders added."
  },
  "settings.rebuild_accepted_check_volume_status_for_completion": {
    "zh": "重建请求已接受，完成情况以卷状态为准",
    "en": "Rebuild accepted; check volume status for completion"
  },
  "settings.rebuild_is_already_in_progress": {
    "zh": "重建已在进行中",
    "en": "Rebuild is already in progress"
  },
  "settings.wait_before_requesting_another_rebuild": {
    "zh": "请稍后再请求重建",
    "en": "Wait before requesting another rebuild"
  },
  "settings.rebuild_queued": {
    "zh": "重建已排队",
    "en": "Rebuild queued"
  },
  "settings.rebuilding": {
    "zh": "正在重建",
    "en": "Rebuilding"
  },
  "settings.not_installed": {
    "zh": "尚未安装",
    "en": "Not installed"
  },
  "settings.service_running": {
    "zh": "服务正在运行",
    "en": "Service running"
  },
  "settings.service_stopped": {
    "zh": "服务已停止",
    "en": "Service stopped"
  },
  "settings.service_unavailable_or_identity_check_failed": {
    "zh": "服务不可用或身份核验失败",
    "en": "Service unavailable or identity check failed"
  },
  "settings.service_version_mismatch_update_required": {
    "zh": "服务版本不匹配，请更新",
    "en": "Service version mismatch; update required"
  },
  "settings.installation_payload_not_prepared": {
    "zh": "安装产物尚未准备",
    "en": "Installation payload not prepared"
  },
  "settings.this_operating_system_is_not_supported": {
    "zh": "当前系统暂不支持",
    "en": "This operating system is not supported"
  },
  "settings.recording_for_the_first_time": {
    "zh": "正在首次记录",
    "en": "Recording for the first time"
  },
  "settings.catching_up_with_changes": {
    "zh": "正在补齐更新",
    "en": "Catching up with changes"
  },
  "settings.published": {
    "zh": "已发布",
    "en": "Published"
  },
  "settings.incomplete_coverage": {
    "zh": "覆盖不完整",
    "en": "Incomplete coverage"
  },
  "settings.files_are_changing_retrying": {
    "zh": "变化较快，正在重试",
    "en": "Files are changing; retrying"
  },
  "settings.index_rebuild_required": {
    "zh": "需要重建记录",
    "en": "Index rebuild required"
  },
  "settings.update_failed": {
    "zh": "更新失败",
    "en": "Update failed"
  },
  "settings.query_timed_out": {
    "zh": "查询超时",
    "en": "Query timed out"
  },
  "settings.initializing": {
    "zh": "正在初始化",
    "en": "Initializing"
  },
  "settings.administrator_authorization_canceled": {
    "zh": "已取消管理员授权",
    "en": "Administrator authorization canceled"
  },
  "settings.operation_completed": {
    "zh": "操作已完成",
    "en": "Operation completed"
  },
  "settings.operation_failed_check_service_status": {
    "zh": "操作失败，请检查服务状态",
    "en": "Operation failed; check service status"
  },
  "settings.outcome_unknown_refresh_status_do_not_repeat_the_same_operation_": {
    "zh": "操作结果尚不明确，请刷新状态，勿重复启动同一操作",
    "en": "Outcome unknown. Refresh status; do not repeat the same operation yet"
  },
  "settings.volume_offline": {
    "zh": "卷已离线",
    "en": "Volume offline"
  },
  "settings.volume_offline_stopping_the_old_index": {
    "zh": "卷已离线，正在停止旧索引",
    "en": "Volume offline; stopping the old index"
  },
  "settings.volume_discovery_failed_keeping_the_previous_catalog": {
    "zh": "卷发现失败，保留上次目录",
    "en": "Volume discovery failed; keeping the previous catalog"
  },
  "settings.some_system_metadata_is_inaccessible": {
    "zh": "部分系统元数据不可访问",
    "en": "Some system metadata is inaccessible"
  },
  "settings.some_files_are_temporarily_in_use": {
    "zh": "部分文件暂被占用",
    "en": "Some files are temporarily in use"
  },
  "settings.some_files_are_being_deleted": {
    "zh": "部分文件正在删除",
    "en": "Some files are being deleted"
  },
  "settings.some_metadata_is_unsupported": {
    "zh": "部分元数据不受支持",
    "en": "Some metadata is unsupported"
  },
  "settings.some_parent_relationships_are_unresolved": {
    "zh": "部分父目录关系尚未补齐",
    "en": "Some parent relationships are unresolved"
  },
  "settings.some_metadata_is_unavailable": {
    "zh": "部分元数据不可用",
    "en": "Some metadata is unavailable"
  },
  "settings.index_coverage_is_incomplete": {
    "zh": "记录覆盖不完整",
    "en": "Index coverage is incomplete"
  },
  "settings.query_unavailable": {
    "zh": "当前查询不可用",
    "en": "Query unavailable"
  },
  "settings.query_reached_its_time_budget": {
    "zh": "查询达到时间预算",
    "en": "Query reached its time budget"
  },
  "settings.volume_change_journal_unavailable": {
    "zh": "卷更新日志暂不可用",
    "en": "Volume change journal unavailable"
  },
  "settings.volume_index_unavailable": {
    "zh": "卷索引暂不可用",
    "en": "Volume index unavailable"
  },
  "settings.volume_index_error": {
    "zh": "卷索引发生错误",
    "en": "Volume index error"
  },
  "settings.volume_index_stopped": {
    "zh": "卷索引已停止",
    "en": "Volume index stopped"
  },
  "settings.volume_count_limit_exceeded": {
    "zh": "超过当前卷数量上限",
    "en": "Volume count limit exceeded"
  },
  "settings.file_system_unsupported": {
    "zh": "文件系统暂不支持",
    "en": "File system unsupported"
  },
  "settings.not_a_fixed_local_volume": {
    "zh": "非固定本机卷",
    "en": "Not a fixed local volume"
  },
  "settings.volume_unavailable": {
    "zh": "卷当前不可访问",
    "en": "Volume unavailable"
  },
  "settings.volume_mapping_or_file_identity_changed": {
    "zh": "卷映射或文件身份变化",
    "en": "Volume mapping or file identity changed"
  },
  "settings.candidate_changed_during_validation": {
    "zh": "候选在复核时发生变化",
    "en": "Candidate changed during validation"
  },
  "settings.no_supported_volume": {
    "zh": "暂无受支持卷",
    "en": "No supported volume"
  },
  "settings.some_records_are_unavailable": {
    "zh": "部分记录当前不可用",
    "en": "Some records are unavailable"
  },
  "settings.maintenance_stopped_stop_and_restart_the_service_under_service_m": {
    "zh": "维护线程已停止；请在服务管理中停止服务后重新启动",
    "en": "Maintenance stopped; stop and restart the service under Service management"
  },
  "settings.index_storage_identity_changed_check_storage_and_repair_the_serv": {
    "zh": "索引存储身份变化；请检查存储并修复服务",
    "en": "Index storage identity changed; check storage and repair the service"
  },
  "settings.volume_identity_changed_refresh_status": {
    "zh": "卷身份变化，请刷新状态",
    "en": "Volume identity changed; refresh status"
  },
  "settings.metadata_is_changing_limited_retries_in_progress": {
    "zh": "元数据正在变化，正在有限重试",
    "en": "Metadata is changing; limited retries in progress"
  },
  "settings.metadata_keeps_changing_try_a_manual_rebuild_later": {
    "zh": "元数据持续变化；可稍后手动重建",
    "en": "Metadata keeps changing; try a manual rebuild later"
  },
  "settings.index_update_failed_try_a_manual_rebuild_and_check_the_service_i": {
    "zh": "索引更新失败；可手动重建，仍失败时检查服务",
    "en": "Index update failed; try a manual rebuild and check the service if it persists"
  },
  "settings.index_update_reached_its_budget_check_the_service_and_retry": {
    "zh": "索引更新达到当前预算；请检查服务后重试",
    "en": "Index update reached its budget; check the service and retry"
  },
  "settings.unmounted_volume": {
    "zh": "未挂载卷",
    "en": "Unmounted volume"
  },
  "settings.not_published_yet": {
    "zh": "尚无已发布时间",
    "en": "Not published yet"
  },
  "settings.unknown_status": {
    "zh": "状态未知",
    "en": "Unknown status"
  },
  "settings.generation_value": {
    "zh": " · 代次 {v0}",
    "en": " · Generation {v0}"
  },
  "settings.stop_and_restart_the_service_under_service_management_then_check": {
    "zh": " · 请在服务管理中停止服务后重新启动，再检查此卷状态",
    "en": " · Stop and restart the service under Service management, then check this volume"
  },
  "settings.record_all_file_names_on_this_volume_again_the_old_generation_re": {
    "zh": "重新记录此卷的所有文件名称？完成前保留旧代次；本操作不修改源文件或系统日志，也不请求管理员权限。",
    "en": "Record all file names on this volume again? The old generation remains until completion. This does not change source files or the system journal and does not require administrator permission."
  },
  "settings.confirm_value_value_only_this_explicit_action_requests_administr": {
    "zh": "确认{v0}？{v1}只有这次明确操作会请求管理员权限。",
    "en": "Confirm: {v0}? {v1} Only this explicit action requests administrator permission."
  },
  "settings.install_the_whole_volume_name_index_component_administrator_perm": {
    "zh": "安装全盘名称索引组件（需要管理员权限）",
    "en": "install the whole-volume name index component (administrator permission required)"
  },
  "settings.update_the_whole_volume_name_index_component_administrator_permi": {
    "zh": "更新全盘名称索引组件（需要管理员权限）",
    "en": "update the whole-volume name index component (administrator permission required)"
  },
  "settings.perform_this_service_management_action": {
    "zh": "执行此服务管理操作",
    "en": "perform this service management action"
  },
  "settings.index_records_will_also_be_deleted_source_files_are_unaffected": {
    "zh": "同时删除索引记录，源文件不受影响。",
    "en": "Index records will also be deleted; source files are unaffected."
  },
  "settings.index_records_are_preserved_by_default": {
    "zh": "索引记录默认保留。",
    "en": "Index records are preserved by default."
  },
  "settings.operation_returned_refresh_status": {
    "zh": "操作已返回，请刷新状态",
    "en": "Operation returned; refresh status"
  },
  "settings.start_the_index_component_under_service_management": {
    "zh": "请在服务管理中启动索引组件",
    "en": "Start the index component under Service management"
  },
  "settings.update_the_service_under_service_management_then_refresh_status": {
    "zh": "请在服务管理中更新服务后刷新状态",
    "en": "Update the service under Service management, then refresh status"
  },
  "settings.refresh_status_if_still_unavailable_check_or_repair_the_service_": {
    "zh": "请刷新状态；持续不可用时检查或修复服务，不会自动提权",
    "en": "Refresh status. If still unavailable, check or repair the service. No automatic elevation"
  },
  "settings.full_access_whole_volume_name_search_by_default": {
    "zh": "完全访问：默认使用全盘名称查询",
    "en": "Full access: whole-volume name search by default"
  },
  "settings.restricted_access_authorized_folders_only": {
    "zh": "受限访问：仅查询已授权目录",
    "en": "Restricted access: authorized folders only"
  },
  "settings.installation_payload_missing_this_build_cannot_install_the_servi": {
    "zh": " · 安装产物尚未准备，本构建不能安装服务",
    "en": " · Installation payload missing; this build cannot install the service"
  },
  "settings.rebuild_index": {
    "zh": "重建索引",
    "en": "Rebuild index"
  },
  "settings.report_copied": {
    "zh": "报告已复制",
    "en": "Report copied"
  },
  "settings.press_ctrl_c_to_copy_the_selected_report": {
    "zh": "请按 Ctrl+C 复制选中的报告",
    "en": "Press Ctrl+C to copy the selected report"
  },
  "home.unknown": {
    "zh": "未知",
    "en": "Unknown"
  },
  "home.end": {
    "zh": "结束",
    "en": "End"
  },
  "home.unavailable": {
    "zh": "无法结束",
    "en": "Unavailable"
  },
  "home.end_confirmed_process_instances_after_local_confirmation": {
    "zh": "结束已确认的进程实例，先本地确认",
    "en": "End confirmed process instances, after local confirmation"
  },
  "home.process": {
    "zh": "进程",
    "en": "Process"
  },
  "home.end_these_value_confirmed_processes_value_value": {
    "zh": "结束以下 {v0} 个已确认进程？{v1}。{v2}",
    "en": "End confirmed processes (count: {v0})? {v1}. {v2}"
  },
  "home.refresh_the_overview_to_check_process_status": {
    "zh": "请刷新概览检查进程状态。",
    "en": "Refresh the overview to check process status."
  },
  "home.unknown_name": {
    "zh": "未知名称",
    "en": "Unknown name"
  },
  "home.observed_value_cpu_window_value_manual_refresh_only_no_trend_his": {
    "zh": "采样时间：{v0} · CPU 计数窗口：{v1} · 仅手动刷新，不记录趋势。",
    "en": "Observed: {v0} · CPU window: {v1} · Manual refresh only; no trend history."
  },
  "home.seconds": {
    "zh": " 秒",
    "en": " s"
  },
  "home.waiting_for_sample": {
    "zh": "等待采样",
    "en": "Waiting for sample"
  },
  "home.enumerated_value_processes_showing_the_top_value_cpu_and_top_val": {
    "zh": "本次枚举 {v0} 个进程，同时显示 CPU 占用前 {v1} 项与内存占用前 {v2} 项，来自同一次采样。{v3} 个进程在两次观测间消失或身份变化，未列入；新出现或无法读取的进程可能缺少 CPU 数据。排序仅比较本次可读值，未知排在末尾。",
    "en": "Processes sampled: {v0}. CPU ranking: {v1}; memory ranking: {v2}, from one sample. Processes lost or changed between observations: {v3}; these are omitted. New or unreadable processes may lack CPU values. Rankings compare readable values only; unknown values appear last."
  },
  "home.unknown_173": {
    "zh": "未知数量",
    "en": "unknown"
  },
  "home.waiting_for_windows_administrator_confirmation": {
    "zh": "等待 Windows 管理员确认…",
    "en": "Waiting for Windows administrator confirmation…"
  },
  "home.requesting_windows": {
    "zh": "正在请求 Windows…",
    "en": "Requesting Windows…"
  },
  "home.actions_for_value": {
    "zh": "{v0}的操作",
    "en": "Actions for {v0}"
  },
  "home.run": {
    "zh": "运行",
    "en": "Run"
  },
  "home.run_as_administrator": {
    "zh": "以管理员身份运行",
    "en": "Run as administrator"
  },
  "home.open_file_location": {
    "zh": "打开文件位置",
    "en": "Open file location"
  },
  "home.uninstall_open_windows_settings": {
    "zh": "卸载（打开系统设置）",
    "en": "Uninstall (open Windows settings)"
  },
  "home.wait_for_the_current_operation_to_finish": {
    "zh": "请等待当前操作完成",
    "en": "Wait for the current operation to finish"
  },
  "home.refresh_the_overview_to_obtain_action_references": {
    "zh": "请刷新概览以获取操作入口",
    "en": "Refresh the overview to obtain action references"
  },
  "home.unavailable_183": {
    "zh": "暂不可用",
    "en": "Unavailable"
  },
  "home.value_right_click_or_press_shift_f10_for_actions": {
    "zh": "{v0}，右键或 Shift+F10 查看操作",
    "en": "{v0}, right-click or press Shift+F10 for actions"
  },
  "home.version_information_not_registered": {
    "zh": "版本信息未登记",
    "en": "Version information not registered"
  },
  "home.running": {
    "zh": "运行中",
    "en": "Running"
  },
  "home.running_state_unknown": {
    "zh": "运行状态未知",
    "en": "Running state unknown"
  },
  "home.estimated_installed_size_value": {
    "zh": "估算安装占用：{v0}",
    "en": "Estimated installed size: {v0}"
  },
  "home.only_value_confirmed_instances": {
    "zh": "仅已确认的 {v0} 个实例",
    "en": "Confirmed instances only: {v0}"
  },
  "home.no_reliable_process_association_cannot_end_safely": {
    "zh": "缺少可靠进程关联，无法安全结束",
    "en": "No reliable process association; cannot end safely"
  },
  "home.no_matching_application_records_in_this_scope": {
    "zh": "当前范围内没有匹配的应用记录。",
    "en": "No matching application records in this scope."
  },
  "home.used_value_total_value_available_value": {
    "zh": "已用 {v0} / 总计 {v1} · 可用 {v2}",
    "en": "Used {v0} / Total {v1} · Available {v2}"
  },
  "home.value_hours": {
    "zh": "{v0} 小时",
    "en": "Hours: {v0}"
  },
  "home.value_value_logical_processors": {
    "zh": "{v0} · {v1} 个逻辑处理器",
    "en": "{v0} · Logical CPUs: {v1}"
  },
  "home.unknown_architecture": {
    "zh": "未知架构",
    "en": "Unknown architecture"
  },
  "home.observed_at_value": {
    "zh": "采样于 {v0}",
    "en": "Observed at {v0}"
  },
  "home.used_space_on_value": {
    "zh": "{v0}已用空间",
    "en": "Used space on {v0}"
  },
  "home.space_usage_unknown": {
    "zh": "容量占用未知",
    "en": "Space usage unknown"
  },
  "home.available_value_value": {
    "zh": "可用 {v0} / {v1}",
    "en": "Available {v0} / {v1}"
  },
  "home.no_disk_capacity_data_available_in_this_sample": {
    "zh": "本次没有可用的磁盘容量数据。",
    "en": "No disk capacity data available in this sample."
  },
  "home.value": {
    "zh": "· {v0} 个",
    "en": "· {v0}"
  },
  "home.ip_configured": {
    "zh": "已配置 IP",
    "en": "IP configured"
  },
  "home.no_ip_observed": {
    "zh": "未观测到 IP",
    "en": "No IP observed"
  },
  "home.value_records": {
    "zh": "· {v0} 项记录",
    "en": "· Records: {v0}"
  },
  "home.unavailable_205": {
    "zh": "· 暂不可用",
    "en": "· Unavailable"
  },
  "home.value_records_206": {
    "zh": "{v0} 项记录",
    "en": "Records: {v0}"
  },
  "home.value_known_running_apps_unknown_does_not_mean_stopped": {
    "zh": "{v0} 个已知运行应用 · 未确认状态不等于未运行",
    "en": "Known running: {v0} · Unknown does not mean stopped"
  },
  "home.registration_and_running_state_not_yet_confirmed": {
    "zh": "登记与运行情况尚不可确认",
    "en": "Registration and running state not yet confirmed"
  },
  "home.applications_observed_at_value": {
    "zh": " 应用采样于 {v0}。",
    "en": " Applications observed at {v0}."
  },
  "home.value_process_paths_unreadable_possibly_due_to_os_permissions": {
    "zh": " {v0} 个进程路径不可读，可能受系统权限限制。",
    "en": " Unreadable process paths: {v0}, possibly due to OS permissions."
  },
  "home.some_registry_locations_were_unreadable_or_the_enumeration_limit": {
    "zh": " 部分注册表位置读取失败或达到枚举上限。",
    "en": " Some registry locations were unreadable or the enumeration limit was reached."
  },
  "home.reading_this_computer_s_overview": {
    "zh": "正在读取本机概览…",
    "en": "Reading this computer's overview…"
  },
  "home.computer_overview_updated": {
    "zh": "本机概览已更新",
    "en": "Computer overview updated"
  },
  "home.refresh_failed_keeping_the_previous_dated_sample_below": {
    "zh": "刷新失败，下面保留上一次采样。",
    "en": "Refresh failed; keeping the previous dated sample below."
  },
  "home.overview_unavailable_refresh_again_later": {
    "zh": "本次概览暂不可用，请稍后刷新。",
    "en": "Overview unavailable. Refresh again later."
  },
  "home.volume_value_value": {
    "zh": "卷 {v0}：{v1}",
    "en": "Volume {v0}: {v1}"
  },
  "home.brand_value": {
    "zh": "品牌：{v0}",
    "en": "Brand: {v0}"
  },
  "home.model_value": {
    "zh": "型号：{v0}",
    "en": "Model: {v0}"
  },
  "home.operating_system_value": {
    "zh": "操作系统：{v0}",
    "en": "Operating system: {v0}"
  },
  "home.total_memory_value": {
    "zh": "总内存：{v0}",
    "en": "Total memory: {v0}"
  },
  "home.display_adapters_value": {
    "zh": "显示适配器：{v0}",
    "en": "Display adapters: {v0}"
  },
  "home.volume_capacities_not_physical_disks_do_not_add_value": {
    "zh": "卷容量（不是物理硬盘，不累加）：{v0}",
    "en": "Volume capacities (not physical disks; do not add): {v0}"
  },
  "home.observed_at_value_223": {
    "zh": "采集时间：{v0}",
    "en": "Observed at: {v0}"
  },
  "home.wait_for_the_current_operation_to_finish_before_reading_the_prof": {
    "zh": "请等待当前操作完成后读取档案。",
    "en": "Wait for the current operation to finish before reading the profile."
  },
  "home.reading_this_computer_s_profile": {
    "zh": "正在读取本机电脑档案…",
    "en": "Reading this computer's profile…"
  },
  "home.read_complete_copy_includes_only_the_summary_below_without_host_": {
    "zh": "已读取；复制仅包含下方摘要，不含主机名、用户名、序列号、地址或私人路径。",
    "en": "Read complete. Copy includes only the summary below, without host name, user name, serial numbers, addresses or private paths."
  },
  "home.read_failed_valuevalue": {
    "zh": "读取失败：{v0}{v1}",
    "en": "Read failed: {v0}{v1}"
  },
  "home.keeping_the_previous_summary_and_observation_time": {
    "zh": "；保留上次摘要和采集时间。",
    "en": "; keeping the previous summary and observation time."
  },
  "home.read_the_computer_profile_first": {
    "zh": "请先读取电脑档案。",
    "en": "Read the computer profile first."
  },
  "home.clipboard_unavailable_select_and_copy_the_summary_below_manually": {
    "zh": "当前环境不支持剪贴板，请选择下方摘要手动复制",
    "en": "Clipboard unavailable. Select and copy the summary below manually"
  },
  "home.summary_copied": {
    "zh": "摘要已复制。",
    "en": "Summary copied."
  },
  "home.copy_failed_value": {
    "zh": "复制失败：{v0}",
    "en": "Copy failed: {v0}"
  },
  "ui.your_computer_one_request_away": {
    "zh": "电脑，从一句话开始。",
    "en": "Your computer, one request away."
  },
  "ui.new_session": {
    "zh": "新建会话",
    "en": "New session"
  },
  "ui.new_session_235": {
    "zh": "＋ 新会话",
    "en": "＋ New session"
  },
  "ui.sessions": {
    "zh": "会话列表",
    "en": "Sessions"
  },
  "ui.app_settings": {
    "zh": "应用设置",
    "en": "App settings"
  },
  "ui.resize_sidebar": {
    "zh": "调整侧栏宽度",
    "en": "Resize sidebar"
  },
  "ui.drag_to_resize_double_click_to_reset_or_use_the_left_right_arrow": {
    "zh": "拖动调整宽度，双击恢复；也可用左右方向键",
    "en": "Drag to resize; double-click to reset, or use the left/right arrow keys"
  },
  "ui.workspace_tabs": {
    "zh": "工作区标签",
    "en": "Workspace tabs"
  },
  "ui.local_observation_clear_scope": {
    "zh": "◇ 本地观察，明确范围",
    "en": "◇ Local observation, clear scope"
  },
  "ui.retry_save": {
    "zh": "重试保存",
    "en": "Retry save"
  },
  "ui.understand_your_computer_now": {
    "zh": "此刻，了解你的电脑",
    "en": "Understand your computer now"
  },
  "ui.see_what_matters_at_a_glance": {
    "zh": "一眼看见，心中有数。",
    "en": "See what matters at a glance."
  },
  "ui.reading_local_status": {
    "zh": "正在读取本机状态…",
    "en": "Reading local status…"
  },
  "ui.refresh_overview": {
    "zh": "刷新概览",
    "en": "Refresh overview"
  },
  "ui.cpu_usage": {
    "zh": "CPU 使用率",
    "en": "CPU usage"
  },
  "ui.whole_computer_short_sample": {
    "zh": "整机口径 · 短时采样",
    "en": "Whole computer · Short sample"
  },
  "ui.view_resource_details": {
    "zh": "查看资源详情 ↗",
    "en": "View resource details ↗"
  },
  "ui.memory": {
    "zh": "内存",
    "en": "Memory"
  },
  "ui.system_uptime": {
    "zh": "系统已运行",
    "en": "System uptime"
  },
  "ui.applications": {
    "zh": "应用概况",
    "en": "Applications"
  },
  "ui.view_registered_apps": {
    "zh": "查看已登记应用 ↗",
    "en": "View registered apps ↗"
  },
  "ui.disk_space": {
    "zh": "磁盘空间",
    "en": "Disk space"
  },
  "ui.not_sampled_yet": {
    "zh": "尚未采样",
    "en": "Not sampled yet"
  },
  "ui.resource_details": {
    "zh": "资源占用详情",
    "en": "Resource details"
  },
  "ui.top_cpu_processes": {
    "zh": "CPU 占用前列",
    "en": "Top CPU processes"
  },
  "ui.process_pid": {
    "zh": "进程 / PID",
    "en": "Process / PID"
  },
  "ui.cpu_whole_computer": {
    "zh": "CPU（整机）",
    "en": "CPU (whole computer)"
  },
  "ui.action": {
    "zh": "操作",
    "en": "Action"
  },
  "ui.no_cpu_process_details_available_in_this_sample": {
    "zh": "本次没有可用的 CPU 进程详情。",
    "en": "No CPU process details available in this sample."
  },
  "ui.top_memory_processes": {
    "zh": "内存占用前列",
    "en": "Top memory processes"
  },
  "ui.no_memory_process_details_available_in_this_sample": {
    "zh": "本次没有可用的内存进程详情。",
    "en": "No memory process details available in this sample."
  },
  "ui.no_process_details_available_in_this_sample": {
    "zh": "本次没有可用的进程详情。",
    "en": "No process details available in this sample."
  },
  "ui.cpu_is_measured_against_the_whole_computer_s_capacity_100_memory": {
    "zh": "CPU 以整机总容量为 100%，内存是采样结束时的进程占用。未知不等于零；进程占用不能相加解释全部内存，内核、缓存与共享内存归属未覆盖。单次高占用不能确定卡顿原因。",
    "en": "CPU is measured against the whole computer's capacity (100%). Memory is process usage at sample end. Unknown does not mean zero. Process values do not account for all memory; kernel, cache and shared memory attribution are not covered. One high sample cannot identify the cause of a slowdown."
  },
  "ui.ask_the_model_to_explain_this_snapshot": {
    "zh": "让模型解释这次资源快照 ↗",
    "en": "Ask the model to explain this snapshot ↗"
  },
  "ui.only_when_clicked_sends_this_cpu_memory_and_disk_snapshot_plus_t": {
    "zh": "仅在点击后发送本次 CPU、内存、磁盘及 CPU / 内存各前 10 个进程的名称、PID 与占用值（两种排序均包含）。不发送应用清单；无需模型也能查看详情。",
    "en": "Only when clicked, sends this CPU, memory and disk snapshot plus the names, PIDs and usage of both CPU and memory top-ten lists. Does not send the app catalog. You can view details without a model."
  },
  "ui.computer_profile": {
    "zh": "电脑档案",
    "en": "Computer profile"
  },
  "ui.system_provided_configuration_summary_missing_values_remain_unkn": {
    "zh": "系统提供的配置摘要；缺失项显示未知。磁盘按卷列出，不代表物理硬盘，不累加；实时可用空间见上方磁盘卡片。显示适配器可能包含虚拟设备。仅展开时读取，在本次窗口内缓存。",
    "en": "System-provided configuration summary; missing values remain unknown. Disks are volumes, not physical drives; do not add their capacities. See disk cards above for current free space. Display adapters may include virtual devices. Read only when expanded and cached for this window."
  },
  "ui.read_refresh_profile": {
    "zh": "读取 / 刷新档案",
    "en": "Read / refresh profile"
  },
  "ui.copy_summary": {
    "zh": "复制摘要",
    "en": "Copy summary"
  },
  "ui.not_read_yet": {
    "zh": "尚未读取",
    "en": "Not read yet"
  },
  "ui.network_interfaces": {
    "zh": "网络接口",
    "en": "Network interfaces"
  },
  "ui.registered_apps": {
    "zh": "已登记应用",
    "en": "Registered apps"
  },
  "ui.filter_apps": {
    "zh": "筛选应用",
    "en": "Filter apps"
  },
  "ui.filter_apps_by_name": {
    "zh": "按名称筛选应用",
    "en": "Filter apps by name"
  },
  "ui.app_display_mode": {
    "zh": "应用显示方式",
    "en": "App display mode"
  },
  "ui.cards": {
    "zh": "卡片",
    "en": "Cards"
  },
  "ui.list": {
    "zh": "列表",
    "en": "List"
  },
  "ui.the_overview_reads_this_computer_directly_without_calling_a_mode": {
    "zh": "概览直接从本机读取，不自动调用模型。短时采样不能单独确定卡顿根因。",
    "en": "The overview reads this computer directly without calling a model. A short sample alone cannot identify a slowdown's root cause."
  },
  "ui.visible_history_is_preserved_model_context_has_restarted_old_ref": {
    "zh": "可见历史已保留；模型执行上下文已重新开始，旧指代与文件候选不可沿用，请重新描述或搜索。",
    "en": "Visible history is preserved; model context has restarted. Old references and file candidates cannot be reused. Describe or search again."
  },
  "ui.conversation": {
    "zh": "对话",
    "en": "Conversation"
  },
  "ui.one_less_step": {
    "zh": "少想一步，轻松一点",
    "en": "One less step"
  },
  "ui.what_are_you_looking_for": {
    "zh": "想找什么？",
    "en": "What are you looking for?"
  },
  "ui.describe_the_file_name_project_or_clues_you_remember": {
    "zh": "描述文件的名字、所在项目，或你记得的线索。",
    "en": "Describe the file name, project, or clues you remember."
  },
  "ui.help_me_find_pdf_documents": {
    "zh": "帮我找一下 PDF 文档",
    "en": "Help me find PDF documents"
  },
  "ui.find_pdf_documents": {
    "zh": "找一下 PDF 文档 ↗",
    "en": "Find PDF documents ↗"
  },
  "ui.find_the_client_initialization_sql_scripts": {
    "zh": "找一下客户端初始化的 SQL 脚本",
    "en": "Find the client initialization SQL scripts"
  },
  "ui.find_initialization_scripts": {
    "zh": "找到初始化脚本 ↗",
    "en": "Find initialization scripts ↗"
  },
  "ui.find_real_candidates_first_then_decide_what_to_do": {
    "zh": "先找到真实候选，再决定下一步。",
    "en": "Find real candidates first, then decide what to do."
  },
  "ui.file_candidates": {
    "zh": "文件候选",
    "en": "File candidates"
  },
  "ui.files_found": {
    "zh": "找到的文件",
    "en": "Files found"
  },
  "ui.confirm_open": {
    "zh": "确认打开",
    "en": "Confirm open"
  },
  "ui.your_confirmation_is_needed": {
    "zh": "需要你确认",
    "en": "Your confirmation is needed"
  },
  "ui.open_this_file": {
    "zh": "打开这个文件？",
    "en": "Open this file?"
  },
  "ui.use_the_system_default_application_this_request_expires_after_90": {
    "zh": "交给系统默认应用打开。此请求将在 90 秒后失效。",
    "en": "Use the system default application. This request expires after 90 seconds."
  },
  "ui.not_now": {
    "zh": "暂不打开",
    "en": "Not now"
  },
  "ui.confirm_open_298": {
    "zh": "确认打开 ↗",
    "en": "Confirm open ↗"
  },
  "ui.make_it_yours": {
    "zh": "按你的习惯使用",
    "en": "Make it yours"
  },
  "ui.connect_models_and_choose_access_settings_are_shared_across_entr": {
    "zh": "连接模型，选择范围。你的设置由各入口共用。",
    "en": "Connect models and choose access. Settings are shared across entry points."
  },
  "ui.settings_categories": {
    "zh": "设置分类",
    "en": "Settings categories"
  },
  "ui.models": {
    "zh": "模型",
    "en": "Models"
  },
  "ui.access": {
    "zh": "访问权限",
    "en": "Access"
  },
  "ui.file_search": {
    "zh": "文件搜索",
    "en": "File search"
  },
  "ui.general_appearance": {
    "zh": "通用与外观",
    "en": "General & appearance"
  },
  "ui.default_model": {
    "zh": "默认模型",
    "en": "Default model"
  },
  "ui.used_for_future_requests": {
    "zh": "用于后续请求",
    "en": "Used for future requests"
  },
  "ui.saving_preserves_visible_history_and_restarts_model_context_old_": {
    "zh": "保存后保留可见历史，重新开始模型上下文，旧候选失效。",
    "en": "Saving preserves visible history and restarts model context. Old candidates expire."
  },
  "ui.my_models": {
    "zh": "我的模型",
    "en": "My models"
  },
  "ui.added_models": {
    "zh": "已添加的模型",
    "en": "Added models"
  },
  "ui.add_model": {
    "zh": "＋ 添加模型",
    "en": "＋ Add model"
  },
  "ui.remove_model": {
    "zh": "移除模型",
    "en": "Remove model"
  },
  "ui.provider": {
    "zh": "提供方",
    "en": "Provider"
  },
  "ui.custom": {
    "zh": "自定义",
    "en": "Custom"
  },
  "ui.api_endpoint": {
    "zh": "API 地址",
    "en": "API endpoint"
  },
  "ui.full_chat_completions_endpoint": {
    "zh": "完整的 Chat Completions 地址",
    "en": "Full Chat Completions endpoint"
  },
  "ui.model_name": {
    "zh": "模型名称",
    "en": "Model name"
  },
  "ui.provider_s_model_id": {
    "zh": "提供方的模型 ID",
    "en": "Provider's model ID"
  },
  "ui.enter_a_model_id": {
    "zh": "填写模型名称",
    "en": "Enter a model ID"
  },
  "ui.not_set": {
    "zh": "尚未设置",
    "en": "Not set"
  },
  "ui.leave_blank_to_keep_the_current_key": {
    "zh": "留空保留已有密钥",
    "en": "Leave blank to keep the current key"
  },
  "ui.save_in_windows_credential_manager": {
    "zh": "保存到 Windows 凭据管理器",
    "en": "Save in Windows Credential Manager"
  },
  "ui.more_settings": {
    "zh": "更多设置",
    "en": "More settings"
  },
  "ui.api_key_environment_variable": {
    "zh": "密钥环境变量",
    "en": "API key environment variable"
  },
  "ui.existing_environment_variables_take_precedence_over_stored_keys_": {
    "zh": "已有环境变量优先于系统保存的密钥。新输入密钥只更新此模型；本机免密模型可留空。",
    "en": "Existing environment variables take precedence over stored keys. A newly entered key updates only this model. Keyless local models can leave it blank."
  },
  "ui.test_this_model": {
    "zh": "测试此模型",
    "en": "Test this model"
  },
  "ui.uses_synthetic_examples_up_to_two_requests_which_may_incur_charg": {
    "zh": "使用虚构样例，最多两次请求，可能产生费用；不发送本机文件。",
    "en": "Uses synthetic examples, up to two requests, which may incur charges. Local files are not sent."
  },
  "ui.access_mode": {
    "zh": "访问模式",
    "en": "Access mode"
  },
  "ui.file_access": {
    "zh": "文件访问",
    "en": "File access"
  },
  "ui.restricted_selected_folders_only": {
    "zh": "安全模式 · 仅指定目录",
    "en": "Restricted · Selected folders only"
  },
  "ui.full_access_locations_available_to_the_current_os_user": {
    "zh": "完全访问 · 当前系统用户可访问的位置",
    "en": "Full access · Locations available to the current OS user"
  },
  "ui.search_folders": {
    "zh": "查找目录",
    "en": "Search folders"
  },
  "ui.choose_search_folder": {
    "zh": "＋ 选择查找目录",
    "en": "＋ Choose search folder"
  },
  "ui.cli_writable_folders": {
    "zh": "CLI 可写目录",
    "en": "CLI writable folders"
  },
  "ui.choose_writable_folder": {
    "zh": "＋ 选择可写目录",
    "en": "＋ Choose writable folder"
  },
  "ui.folder_choices_modify_the_draft_save_to_apply_full_access_does_n": {
    "zh": "目录选择只修改草稿，保存后生效。完全访问不提升系统权限，也不跳过确认。桌面当前只提供查找、元数据与确认打开；移动仍通过 CLI 执行并记录。新模式不自动开放删除或覆盖。",
    "en": "Folder choices modify the draft; save to apply. Full access does not elevate OS permissions or bypass confirmations. Desktop provides search, metadata and opening; moves still use the CLI with a journal. No automatic delete or overwrite permission."
  },
  "ui.whole_volume_quick_search": {
    "zh": "全盘快速搜索",
    "en": "Whole-volume quick search"
  },
  "ui.records_names_on_supported_fixed_local_ntfs_volumes_without_sele": {
    "zh": "自动记录受支持的本机固定 NTFS 卷名称，无需逐目录登记，不读取正文。完全访问时默认使用。首次需要明确安装索引组件，此后桌面保持普通权限；启动与搜索不会自行弹出授权。",
    "en": "Records names on supported fixed local NTFS volumes without selecting each folder or reading file contents. Used by default with Full access. Initial component installation requires an explicit decision; the desktop then keeps ordinary permissions. Startup and search never request elevation automatically."
  },
  "ui.finding_a_name_does_not_authorize_inspecting_or_opening_it_actio": {
    "zh": "名称可见不代表已获准查看或打开。后续操作仍遵循访问权限设置。点击当前候选直接交系统默认应用打开。复杂路径、时间、大小等筛选使用明确列出的兼容目录补充，不会忽略条件。",
    "en": "Finding a name does not authorize inspecting or opening it. Actions still follow access settings. Clicking a current candidate opens it with the system default application. Complex path, time or size filters supplement results only within the stated compatible folders; conditions are never ignored."
  },
  "ui.service_status_not_read_yet": {
    "zh": "尚未读取服务状态",
    "en": "Service status not read yet"
  },
  "ui.refresh_status": {
    "zh": "刷新状态",
    "en": "Refresh status"
  },
  "ui.generate_diagnostic_report": {
    "zh": "生成诊断报告",
    "en": "Generate diagnostic report"
  },
  "ui.safe_diagnostic_report_no_private_file_paths_or_account_identifi": {
    "zh": "安全诊断报告（不含私人文件路径与账户标识）",
    "en": "Safe diagnostic report (no private file paths or account identifiers)"
  },
  "ui.index_diagnostic_report": {
    "zh": "索引诊断报告",
    "en": "Index diagnostic report"
  },
  "ui.copy_report": {
    "zh": "复制报告",
    "en": "Copy report"
  },
  "ui.service_management": {
    "zh": "服务管理",
    "en": "Service management"
  },
  "ui.these_actions_request_administrator_permission_only_after_clicki": {
    "zh": "以下操作只在点击并确认后请求管理员权限。停止服务保留旧索引；卸载默认保留索引数据。",
    "en": "These actions request administrator permission only after clicking and confirming. Stopping preserves the old index; uninstalling preserves index data by default."
  },
  "ui.install_index_component": {
    "zh": "安装索引组件",
    "en": "Install index component"
  },
  "ui.start_service": {
    "zh": "启动服务",
    "en": "Start service"
  },
  "ui.stop_service": {
    "zh": "停止服务",
    "en": "Stop service"
  },
  "ui.update_service": {
    "zh": "更新服务",
    "en": "Update service"
  },
  "ui.repair_service": {
    "zh": "修复服务",
    "en": "Repair service"
  },
  "ui.roll_back_previous_version": {
    "zh": "回滚上一版本",
    "en": "Roll back previous version"
  },
  "ui.also_delete_index_records_when_uninstalling_source_files_remain": {
    "zh": "卸载时同时删除索引记录（不会删除源文件）",
    "en": "Also delete index records when uninstalling (source files remain)"
  },
  "ui.uninstall_service": {
    "zh": "卸载服务",
    "en": "Uninstall service"
  },
  "ui.accelerated_compatible_folders": {
    "zh": "兼容目录加速搜索",
    "en": "Accelerated compatible folders"
  },
  "ui.existing_folder_records_are_preserved_without_automatic_migratio": {
    "zh": "保留已有目录记录，不自动迁移或删除。复杂筛选和全盘服务不可用时，只在回执列出的兼容目录中补充查询。只记录名称、路径和元数据，不读取正文。目录选择立即生效，不改变访问权限；应用运行时辅助更新。",
    "en": "Existing folder records are preserved without automatic migration or deletion. Complex filters and service outages search only the compatible folders listed in receipts. Records contain names, paths and metadata, never file contents. Folder selection applies immediately without changing access permissions; updates run while the app is open."
  },
  "ui.add_accelerated_folder": {
    "zh": "＋ 添加加速目录",
    "en": "＋ Add accelerated folder"
  },
  "ui.appearance": {
    "zh": "外观",
    "en": "Appearance"
  },
  "ui.theme": {
    "zh": "主题",
    "en": "Theme"
  },
  "ui.follow_system": {
    "zh": "跟随系统",
    "en": "Follow system"
  },
  "ui.light": {
    "zh": "浅色",
    "en": "Light"
  },
  "ui.dark": {
    "zh": "深色",
    "en": "Dark"
  },
  "ui.changing_settings_pages_preserves_the_current_conversation": {
    "zh": "设置页切换不会丢失当前对话。",
    "en": "Changing settings pages preserves the current conversation."
  },
  "ui.discard_changes_and_reload": {
    "zh": "放弃修改并重新载入",
    "en": "Discard changes and reload"
  },
  "ui.save_settings": {
    "zh": "保存设置",
    "en": "Save settings"
  },
  "ui.stop_current_request": {
    "zh": "停止当前请求",
    "en": "Stop current request"
  },
  "ui.which_file_are_you_looking_for": {
    "zh": "你想找什么文件",
    "en": "Which file are you looking for?"
  },
  "ui.find_a_file_in_one_sentence_e_g_find_the_contract_pdf_in_my_proj": {
    "zh": "用一句话找到文件，比如：找一下项目里的合同 PDF",
    "en": "Find a file in one sentence, e.g. find the contract PDF in my project"
  },
  "ui.reading_model_settings": {
    "zh": "正在读取模型配置",
    "en": "Reading model settings"
  },
  "ui.stop": {
    "zh": "停止",
    "en": "Stop"
  },
  "ui.send": {
    "zh": "发送 ↑",
    "en": "Send ↑"
  },
  "ui.enter_to_send_shift_enter_for_a_new_line_search_reads_names_and_": {
    "zh": "Enter 发送 · Shift + Enter 换行 · 文件查找只读取名称与元数据",
    "en": "Enter to send · Shift + Enter for a new line · Search reads names and metadata only"
  },
  "ui.keep": {
    "zh": "保留",
    "en": "Keep"
  },
  "ui.confirm": {
    "zh": "确认",
    "en": "Confirm"
  }
};
messages['language.title']={zh:'语言',en:'Language'};
messages['language.interface']={zh:'界面语言',en:'Interface language'};
messages['language.auto']={zh:'跟随系统',en:'Follow system'};
messages['language.note']={zh:'跟随系统时，中文系统使用简体中文，其余使用英文；手动选择优先。保存后生效。',en:'When following system, Chinese Windows UI uses Simplified Chinese; other languages use English. A manual choice takes priority. Save to apply.'};
messages['error.unknown']={zh:'操作未完成。请重试；若仍失败，请检查设置或诊断报告。',en:'The operation did not complete. Retry; if it persists, check settings or the diagnostic report.'};
messages['error.detail']={zh:'操作未完成：{detail}',en:'The operation did not complete: {detail}'};

const backendMessages = {
  "backend.0": {
    "zh": "接口计数器的短时采样；已配置 IP 不代表互联网可用，不主动发起联网探测。",
    "en": "Short sample of interface counters. A configured IP does not prove internet access; no active connectivity probes are made."
  },
  "backend.1": {
    "zh": "当前平台尚未实现应用清单。",
    "en": "Application inventory is not implemented on this platform."
  },
  "backend.2": {
    "zh": "安装占用为注册表登记估算值，不含完整磁盘扫描。运行中仅确认独占登记的可执行图标完整路径与可读进程路径一致；共享路径、缺少登记、无匹配或权限不足均为未知，不代表未运行。仅枚举当前用户和本机 32/64 位卸载注册表记录；可能含组件，不完整覆盖商店应用或便携应用。",
    "en": "Installed size is a registry estimate, without a full disk scan. Running state requires an exact, uniquely registered executable icon path matching a readable process path. Shared paths, missing records, no match and insufficient permissions remain unknown, not stopped. Only current-user and machine 32/64-bit uninstall registry records are enumerated; components may be included, and Store or portable apps are not fully covered."
  },
  "backend.3": {
    "zh": " 桌面也按唯一已核验的开始菜单程序完整路径关联当前用户可核验实例；结束只处理确认中列出的集合，未知关联不可操作。",
    "en": " The desktop also associates verifiable current-user instances with a unique, validated Start-menu target path. Ending processes affects only the frozen confirmation list; unknown associations cannot be acted on."
  },
  "backend.4": {
    "zh": "短时采样；cpu_percent_total 按整机 100% 计，cpu_percent_one_core 按单核 100% 计。缺失值为 null。进程列表只含所列项，不能据此解释全部已用内存；未覆盖内核、缓存及共享内存归属。不包括 GPU、磁盘 I/O、历史活动或未保存状态，不足以确定卡顿根因。",
    "en": "Short sample. cpu_percent_total uses whole-computer capacity; cpu_percent_one_core uses one core. Missing values are null. The listed processes cannot explain all used memory; kernel, cache and shared-memory attribution are not covered. GPU, disk I/O, historical activity and unsaved state are excluded, so this cannot identify a slowdown's root cause."
  },
  "backend.5": {
    "zh": "全盘名称索引：仅展示当前系统用户可见的候选；按服务快照顺序显示，完整性与时间见各卷回执。搜索覆盖不授予正文读取或打开权限，实际操作仍按访问设置重新核验。",
    "en": "Whole-volume name index: only candidates visible to the current OS user are shown, in service snapshot order. See volume receipts for coverage and time. Search does not grant permission to read contents or open files; actions recheck access settings."
  },
  "backend.6": {
    "zh": "全盘索引服务当前不可用；仅使用列出的已记录兼容目录快照，非全盘；候选仍重新检查权限与身份。",
    "en": "The whole-volume service is unavailable. Searching only the listed compatible folder snapshots, not whole-volume. Candidate permissions and identity are rechecked."
  },
  "backend.7": {
    "zh": "当前筛选或页码使用列出的已记录兼容目录快照，非全盘；所有筛选与排序仍保留，候选重新检查权限与身份。",
    "en": "These filters or page use only the listed compatible folder snapshots, not whole-volume. All filters and sorting are preserved; candidate permissions and identity are rechecked."
  },
  "backend.8": {
    "zh": "全盘索引服务当前不可用；本次仅在列出的授权兼容目录内补充查询，非全盘。最多 5 秒、100000 项、32 层；可在设置检查服务，不会自动请求管理员权限。",
    "en": "The whole-volume service is unavailable. Searching only the listed authorized compatible folders, not whole-volume. Limits: 5 seconds, 100000 entries, 32 levels. Check the service in Settings; administrator permission is never requested automatically."
  },
  "backend.9": {
    "zh": "目录补充查询：本次条件只在列出的授权目录内按全部筛选执行，非全盘；最多 5 秒、100000 项、32 层，不读取正文。",
    "en": "Supplementary folder search: all conditions apply only within the listed authorized folders, not whole-volume. Limits: 5 seconds, 100000 entries, 32 levels. File contents are not read."
  },
  "backend.10": {
    "zh": "仅在列出的目录内扫描文件名、相对路径和元数据；不读取正文，不能据文件名确认用途；跳过链接/不可访问项，翻页时重新观察。",
    "en": "Scans names, relative paths and metadata only within the listed folders. Does not read contents or infer purpose from names. Links and inaccessible items are skipped; changing pages observes files again."
  },
  "backend.11": {
    "zh": "已记录目录：按上次完整快照匹配和排序，非实时；后续新增/修改可能尚未收录；应用运行时辅助更新。候选已重新检查权限与身份，失效项跳过。可在设置 → 文件搜索立即更新。",
    "en": "Recorded folders: matching and sorting use the last complete snapshot, not live data. Recent additions or changes may be missing; updates run while the app is open. Candidate permissions and identity are rechecked, and stale items skipped. Update now in Settings → File search."
  },
  "backend.12": {
    "zh": "现场扫描：此筛选需要现场观察，未忽略任何筛选条件。",
    "en": "Live scan: these filters require a new observation; no filter conditions were ignored."
  },
  "backend.13": {
    "zh": "系统已接受打开请求；尚未验证应用窗口或文件显示状态",
    "en": "Windows accepted the open request; the app window or displayed file has not been verified"
  },
  "backend.14": {
    "zh": "用户未选择打开；没有提交请求",
    "en": "Opening was declined; no request was submitted"
  },
  "backend.15": {
    "zh": "请求已交给 Windows；不代表应用已启动或卸载已完成。",
    "en": "Request handed to Windows; this does not prove the app started or uninstall completed."
  },
  "backend.16": {
    "zh": "操作已取消，未报告成功。",
    "en": "Operation canceled; success is not reported."
  },
  "backend.17": {
    "zh": "缺少可靠的同用户进程身份或唯一程序关联；无法安全结束，请刷新概览",
    "en": "No reliable same-user process identity or unique app association; cannot end safely. Refresh the overview"
  },
  "backend.18": {
    "zh": "强制结束可能丢失未保存的内容；只结束此处列出的进程，不自动结束其他实例或子进程。",
    "en": "Forcing exit may lose unsaved work. Only these listed processes will end; other instances and child processes are not ended automatically."
  },
  "backend.19": {
    "zh": "已取消，未结束进程。",
    "en": "Canceled; no process was ended."
  },
  "backend.20": {
    "zh": "已退出",
    "en": "Exited"
  },
  "backend.21": {
    "zh": "原进程已退出",
    "en": "Original process already exited"
  },
  "backend.22": {
    "zh": "已请求结束，仍在等待退出",
    "en": "End requested; still waiting for exit"
  },
  "backend.23": {
    "zh": "身份已变化，未操作",
    "en": "Identity changed; no action taken"
  },
  "backend.24": {
    "zh": "已取消，未操作",
    "en": "Canceled; no action taken"
  },
  "backend.25": {
    "zh": "Windows 拒绝访问，未报告退出",
    "en": "Windows denied access; exit is not reported"
  },
  "backend.26": {
    "zh": "受保护进程，未操作",
    "en": "Protected process; no action taken"
  },
  "backend.27": {
    "zh": "非当前用户会话，未操作",
    "en": "Different user or session; no action taken"
  },
  "backend.28": {
    "zh": "身份核验不可用，未操作",
    "en": "Identity check unavailable; no action taken"
  },
  "backend.29": {
    "zh": "未找到唯一同名、无参数的本地开始菜单程序入口，请从 Windows 开始菜单操作",
    "en": "No unique, same-named local Start-menu app target without arguments was found. Use the Windows Start menu"
  },
  "backend.30": {
    "zh": "等待启动校准；当前搜索使用现场扫描",
    "en": "Waiting for startup reconciliation; searches use live scanning"
  },
  "backend.31": {
    "zh": "尚未配置模型，将按文件名搜索；可在设置 → 模型中添加",
    "en": "No model configured. Search will match file names; add a model in Settings → Models."
  },
  "backend.32": {
    "zh": "正在理解请求……",
    "en": "Understanding the request…"
  },
  "backend.33": {
    "zh": "本轮已取消；已经完成的改动不会撤回",
    "en": "Request canceled; completed changes are not rolled back"
  },
  "backend.34": {
    "zh": "此候选已失效，请重新查找",
    "en": "This candidate expired. Search again"
  },
  "backend.35": {
    "zh": "对象已变化，请重新查找",
    "en": "The object changed. Search again"
  },
  "backend.36": {
    "zh": "引用不属于本次会话，请先查找文件",
    "en": "This reference does not belong to this session. Search first"
  },
  "backend.37": {
    "zh": "配置已被其他入口修改，请重新载入后保存",
    "en": "Settings changed in another entry point. Reload before saving"
  },
  "backend.38": {
    "zh": "配置服务不可用",
    "en": "Settings service unavailable"
  },
  "backend.39": {
    "zh": "配置文件过大",
    "en": "Configuration file is too large"
  },
  "backend.40": {
    "zh": "配置格式错误",
    "en": "Configuration format is invalid"
  },
  "backend.41": {
    "zh": "无法读取配置",
    "en": "Could not read configuration"
  },
  "backend.42": {
    "zh": "无法保存配置，原配置保留",
    "en": "Could not save configuration; the original is preserved"
  },
  "backend.43": {
    "zh": "无法准备配置文件，原配置保留",
    "en": "Could not prepare configuration; the original is preserved"
  },
  "backend.44": {
    "zh": "无法写入配置，原配置保留",
    "en": "Could not write configuration; the original is preserved"
  },
  "backend.45": {
    "zh": "无法同步配置，原配置保留",
    "en": "Could not synchronize configuration; the original is preserved"
  },
  "backend.46": {
    "zh": "密钥更新数量过多",
    "en": "Too many API key updates"
  },
  "backend.47": {
    "zh": "密钥所属连接不存在",
    "en": "The API key's connection does not exist"
  },
  "backend.48": {
    "zh": "保存密钥需要设置凭据引用名称",
    "en": "Set a credential reference name before saving a key"
  },
  "backend.49": {
    "zh": "密钥为空或过长",
    "en": "API key is empty or too long"
  },
  "backend.50": {
    "zh": "不支持备用数据流或歧义路径",
    "en": "Alternate data streams or ambiguous paths are unsupported"
  },
  "backend.51": {
    "zh": "不支持设备路径",
    "en": "Device paths are unsupported"
  },
  "backend.52": {
    "zh": "首版只支持本机盘符路径",
    "en": "Only local drive-letter paths are supported"
  },
  "backend.53": {
    "zh": "路径不能包含 ..",
    "en": "Paths cannot contain .."
  },
  "backend.54": {
    "zh": "路径包含空字符",
    "en": "Path contains a null character"
  },
  "backend.55": {
    "zh": "此类型尚未配置 Windows 默认打开应用，请先在系统设置中选择关联应用",
    "en": "No Windows default application is configured for this type. Choose one in Windows settings"
  },
  "backend.56": {
    "zh": "全盘索引服务当前不可用，请在设置 → 文件搜索检查状态；没有兼容目录可补充查询。搜索不会自动请求管理员权限。",
    "en": "The whole-volume index service is unavailable and no compatible search folders are configured. Check Settings → File search. Searching never requests administrator permission automatically."
  },
  "backend.57": {
    "zh": "当前筛选需要目录补充查询，尚无兼容目录。可指定已授权目录，或改用单个文件名关键词查询全盘；不会忽略当前筛选条件。",
    "en": "These filters need supplementary folder search, but no compatible folders are configured. Choose authorized folders or search whole-volume with one file-name keyword. Current filters will not be ignored."
  },
  "backend.58": {
    "zh": "尚未设置读取目录。使用 --read-root 或 daosh config 配置",
    "en": "No search folders configured. Choose folders in Settings → Access"
  },
  "backend.59": {
    "zh": "已有结束进程确认待处理，请先确认或取消",
    "en": "A process confirmation is pending. Confirm or cancel it first"
  },
  "backend.60": {
    "zh": "进程引用已过期，请刷新概览",
    "en": "Process reference expired. Refresh the overview"
  },
  "backend.61": {
    "zh": "进程身份或权限已变化，请刷新概览；尚未结束任何进程",
    "en": "Process identity or permissions changed. Refresh the overview; no process was ended"
  },
  "backend.62": {
    "zh": "结束进程确认已失效，请重新选择",
    "en": "Process confirmation is invalid. Select again"
  },
  "backend.63": {
    "zh": "结束进程确认已过期，未执行",
    "en": "Process confirmation expired; nothing was executed"
  },
  "backend.64": {
    "zh": "应用引用已失效，请刷新概览",
    "en": "Application reference expired. Refresh the overview"
  },
  "backend.65": {
    "zh": "入口不可读取或正在变化，请刷新概览",
    "en": "App target unreadable or changing. Refresh the overview"
  },
  "backend.66": {
    "zh": "入口已变化，请刷新概览",
    "en": "App target changed. Refresh the overview"
  },
  "backend.67": {
    "zh": "请先刷新电脑概览",
    "en": "Refresh the computer overview first"
  },
  "backend.68": {
    "zh": "概览不可用",
    "en": "Overview unavailable"
  },
  "backend.69": {
    "zh": "输入过长",
    "en": "Input is too long"
  },
  "backend.70": {
    "zh": "请输入你想找的文件",
    "en": "Enter the file you want to find"
  },
  "backend.71": {
    "zh": "请输入文件名关键词",
    "en": "Enter a file-name keyword"
  },
  "backend.72": {
    "zh": "单次输入不超过 16KB",
    "en": "A request cannot exceed 16 KB"
  },
  "backend.73": {
    "zh": "请求已失效",
    "en": "Request expired"
  },
  "backend.74": {
    "zh": "请求已结束",
    "en": "Request has ended"
  },
  "backend.75": {
    "zh": "配置已变更，此确认已失效，请重新搜索",
    "en": "Settings changed and this confirmation expired. Search again"
  },
  "backend.76": {
    "zh": "会话不存在",
    "en": "Session does not exist"
  },
  "backend.77": {
    "zh": "会话数量已达 100，请继续使用已有会话",
    "en": "The 100-session limit has been reached. Continue an existing session"
  },
  "backend.78": {
    "zh": "不支持的请求",
    "en": "Unsupported request"
  },
  "backend.79": {
    "zh": "每页 1–50 项，页码 0–100",
    "en": "Use 1–50 items per page and page numbers 0–100"
  },
  "backend.80": {
    "zh": "关键词过长",
    "en": "Search keyword is too long"
  },
  "backend.81": {
    "zh": "大小范围无效",
    "en": "Invalid size range"
  },
  "backend.82": {
    "zh": "时间范围无效",
    "en": "Invalid time range"
  },
  "backend.83": {
    "zh": "不支持的能力；不能执行任意命令",
    "en": "Unsupported capability; arbitrary commands cannot be executed"
  },
  "backend.84": {
    "zh": "本轮打开已取消或结果未知，请重新选择",
    "en": "Opening was canceled or its outcome is unknown. Select again"
  },
  "backend.85": {
    "zh": "上一条请求仍在处理，请先取消或等待完成",
    "en": "The previous request is still running. Cancel it or wait for completion"
  },
  "backend.86": {
    "zh": "确认已过期或已经处理",
    "en": "Confirmation expired or already handled"
  },
  "backend.87": {
    "zh": "确认已过期，未执行",
    "en": "Confirmation expired; nothing was executed"
  },
  "backend.88": {
    "zh": "尚未设置模型",
    "en": "No model configured"
  },
  "backend.89": {
    "zh": "未配置模型",
    "en": "No model configured"
  },
  "backend.90": {
    "zh": "目录不可访问",
    "en": "Folder inaccessible"
  },
  "backend.91": {
    "zh": "（当前无访问权限）",
    "en": "(not currently authorized)"
  },
  "backend.92": {
    "zh": "请选择普通目录",
    "en": "Choose a regular folder"
  },
  "backend.93": {
    "zh": "最多登记 32 个目录",
    "en": "At most 32 folders may be registered"
  },
  "backend.94": {
    "zh": "目录登记文件损坏，无法修改",
    "en": "Folder registry is damaged and cannot be modified"
  },
  "backend.95": {
    "zh": "入口正在退出",
    "en": "The app is closing"
  },
  "backend.96": {
    "zh": "现场扫描：目录记录正在更新、待更新或不可用。",
    "en": "Live scan: folder records are updating, pending or unavailable."
  },
  "backend.97": {
    "zh": "现场扫描：登记目录身份已变化，请重新登记。",
    "en": "Live scan: registered folder identity changed. Register it again."
  },
  "backend.98": {
    "zh": "模型没有返回文字；可以继续提问或使用 /search。",
    "en": "The model returned no text. Ask again or search by file name."
  },
  "backend.99": {
    "zh": "本轮已达到 12 次工具调用上限。实际结果见本地回执；请给出下一步指令。",
    "en": "The 12-tool-call limit was reached. See local receipts for actual results and give the next instruction."
  },
  "backend.100": {
    "zh": "本轮已取消，已完成操作请查看本地回执",
    "en": "Request canceled. See local receipts for completed operations"
  },
  "backend.101": {
    "zh": "安装占用为注册表登记估算值，不含完整磁盘扫描。运行中仅确认独占登记的可执行图标完整路径与可读进程路径一致；共享路径、缺少登记、无匹配或权限不足均为未知，不代表未运行。仅枚举当前用户和本机 32/64 位卸载注册表记录；可能含组件，不完整覆盖商店应用或便携应用。 桌面也按唯一已核验的开始菜单程序完整路径关联当前用户可核验实例；结束只处理确认中列出的集合，未知关联不可操作。",
    "en": "Installed size is a registry estimate, without a full disk scan. Running state requires an exact, uniquely registered executable icon path matching a readable process path. Shared paths, missing records, no match and insufficient permissions remain unknown, not stopped. Only current-user and machine 32/64-bit uninstall registry records are enumerated; components may be included, and Store or portable apps are not fully covered. The desktop also associates verifiable current-user instances with a unique, validated Start-menu target path. Ending processes affects only the frozen confirmation list; unknown associations cannot be acted on."
  }
};
Object.assign(backendMessages,{"provider.endpoint_quotes": {"zh": "endpoint 首尾包含单引号；在 CMD 中请改用双引号，或运行 setup 直接输入地址", "en": "Endpoint has surrounding single quotes. Enter the URL without quotes."}, "provider.connection_missing": {"zh": "模型所属连接不存在", "en": "The model connection does not exist"}, "provider.endpoint_length": {"zh": "模型 endpoint 过长", "en": "Model endpoint is too long"}, "provider.env_length": {"zh": "密钥环境变量名称过长；不要填写 API Key 本身", "en": "API key environment variable name is too long; do not enter the key itself"}, "provider.endpoint_whitespace": {"zh": "模型 endpoint 不能包含空白边界或控制字符", "en": "Model endpoint cannot have surrounding whitespace or control characters"}, "provider.endpoint_url": {"zh": "模型 endpoint 必须为完整 URL", "en": "Model endpoint must be a complete URL"}, "provider.endpoint_host": {"zh": "模型 endpoint 缺少主机地址", "en": "Model endpoint needs a host"}, "provider.endpoint_path": {"zh": "endpoint 只有服务地址；请填写包含 /chat/completions 的完整请求地址", "en": "Enter the full request endpoint, including /chat/completions"}, "provider.https": {"zh": "远程模型必须使用 HTTPS；仅本机服务允许 HTTP", "en": "Remote models require HTTPS; HTTP is allowed only for local services"}, "provider.endpoint_credentials": {"zh": "endpoint 不能包含凭据、查询参数或 fragment", "en": "Endpoint cannot contain credentials, query parameters or fragments"}, "provider.model": {"zh": "模型名称不能为空、超过 256 字节或包含控制字符", "en": "Model name must be nonempty, at most 256 bytes, without control characters"}, "provider.env": {"zh": "密钥环境变量名称只能包含字母、数字和下划线，且不能以数字开头；不要填写 API Key 本身", "en": "API key environment variable names use letters, digits and underscores, cannot start with a digit, and must not contain the key itself"}, "provider.count": {"zh": "连接或模型数量过多", "en": "Too many connections or models"}, "provider.connection_id": {"zh": "连接 ID 重复或无效", "en": "Connection ID is duplicate or invalid"}, "provider.name": {"zh": "连接名称无效", "en": "Connection name is invalid"}, "provider.model_id": {"zh": "模型 ID 重复或无效", "en": "Model ID is duplicate or invalid"}, "control.invalid": {"zh": "此请求已失效", "en": "This request expired"}, "control.ended": {"zh": "此请求已结束", "en": "This request has ended"}, "control.confirm": {"zh": "此确认已失效", "en": "This confirmation expired"}, "control.confirm_open": {"zh": "此确认已失效，请重新请求打开", "en": "This confirmation expired. Request opening again"}, "inventory.watcher": {"zh": "监听不可用；已保留快照，搜索使用现场扫描，稍后重试", "en": "Watching unavailable; snapshot preserved. Search uses live scanning; retry later"}, "inventory.events": {"zh": "事件辅助快照，仍可能有延迟；每 5 分钟校准", "en": "Event-assisted snapshot; delays are possible. Reconciled every 5 minutes"}, "inventory.catchup": {"zh": "正在补齐记录期间收到的变化；此范围暂用现场扫描", "en": "Catching up with changes received while recording; this scope temporarily uses live scanning"}, "language.detection_error": {"zh": "界面语言读取失败，已使用英文；可在设置中选择语言。", "en": "Interface language could not be read; English is used. Choose a language in Settings."}});
Object.assign(backendMessages,{
  "model.timeout": {
    "zh": "模型请求超时；请检查网络或服务负载，已执行操作不会重试",
    "en": "Model request timed out. Check the network or service load; completed actions will not be retried"
  },
  "model.connect": {
    "zh": "无法连接模型服务；请检查地址、端口、代理和服务是否启动",
    "en": "Cannot connect to the model service. Check its address, port, proxy and whether it is running"
  },
  "model.builder": {
    "zh": "无法构造模型请求；请检查密钥格式和本地配置",
    "en": "Cannot construct the model request. Check API key format and local settings"
  },
  "model.connection_failed": {
    "zh": "模型连接失败；请检查网络与服务配置，已执行操作不会重试",
    "en": "Model connection failed. Check network and service settings; completed actions will not be retried"
  },
  "model.read": {
    "zh": "模型响应读取失败；已执行操作不会重试",
    "en": "Failed to read model response; completed actions will not be retried"
  },
  "model.size": {
    "zh": "模型响应超过大小限制",
    "en": "Model response exceeds the size limit"
  },
  "model.format": {
    "zh": "模型未返回兼容的 Chat Completions 工具调用格式",
    "en": "Model did not return a compatible Chat Completions tool-call format"
  },
  "model.empty": {
    "zh": "模型返回空 choices",
    "en": "Model returned empty choices"
  },
  "model.calls": {
    "zh": "模型单次工具调用超限，未执行这批调用",
    "en": "Model returned too many tool calls; this batch was not executed"
  },
  "model.key_format": {
    "zh": "API Key 包含非法字符（内部空白、控制字符或非 ASCII 字符）；请重新复制密钥，首尾空白会自动去除",
    "en": "API key contains invalid whitespace, control or non-ASCII characters. Copy it again; surrounding whitespace is trimmed"
  },
  "model.header": {
    "zh": "API Key 无法构成有效请求头；请重新输入密钥",
    "en": "API key is not a valid request header. Enter it again"
  },
  "model.redirect": {
    "zh": "地址发生重定向，未跟随；请填写最终的完整请求地址",
    "en": "Endpoint redirected and was not followed. Enter the final full request URL"
  },
  "model.incompatible": {
    "zh": "请求格式或参数不兼容；请确认模型支持 Chat Completions 与工具调用",
    "en": "Request format or parameters are incompatible. Verify Chat Completions and tool-call support"
  },
  "model.auth": {
    "zh": "身份验证失败；请检查 API Key 是否有效，可运行 daosh setup 重新输入",
    "en": "Authentication failed. Check the API key in Settings → Models"
  },
  "model.denied": {
    "zh": "服务拒绝访问；请检查账户权限和模型访问权限",
    "en": "Service denied access. Check account and model permissions"
  },
  "model.not_found": {
    "zh": "接口或模型不存在；endpoint 需要包含完整的 /chat/completions 路径",
    "en": "Endpoint or model not found. Include the complete /chat/completions path"
  },
  "model.quota": {
    "zh": "请求受限或额度不足；请检查服务配额，稍后再试",
    "en": "Rate limit or insufficient quota. Check provider limits and retry later"
  },
  "model.server": {
    "zh": "模型服务暂时故障；请稍后再试",
    "en": "Model service temporarily failed. Retry later"
  },
  "model.failed": {
    "zh": "请求未成功；请检查服务兼容性与配置",
    "en": "Request failed. Check service compatibility and settings"
  },
  "model.diagnostic_tool": {
    "zh": "模型可连接，但没有返回单个诊断工具调用；工具调用兼容性尚未通过",
    "en": "Model connected but did not return one diagnostic tool call. Tool-call compatibility has not passed"
  },
  "model.diagnostic_args": {
    "zh": "模型可连接，但诊断工具名称或参数不符合要求；未执行任何工具",
    "en": "Model connected but diagnostic tool name or arguments were invalid. No tool was executed"
  },
  "model.diagnostic_result": {
    "zh": "模型已返回工具调用，但未正确读回诊断结果；工具结果回传兼容性尚未通过",
    "en": "Model returned a tool call but did not read the diagnostic result correctly. Tool-result compatibility has not passed"
  }
});
Object.assign(backendMessages,{
  "inventory.status_0": {
    "zh": "达到 10 万条目 / 60 秒预算，请缩小目录",
    "en": "The 100000-entry / 60-second budget was reached. Narrow the folder scope"
  },
  "inventory.status_1": {
    "zh": "登记目录必须为绝对路径",
    "en": "Registered folders must use absolute paths"
  },
  "inventory.status_2": {
    "zh": "重复目录记录标识",
    "en": "Duplicate folder record identifier"
  },
  "inventory.status_3": {
    "zh": "缓存标识不匹配",
    "en": "Cache identifier mismatch"
  },
  "inventory.status_4": {
    "zh": "缓存目录不匹配",
    "en": "Cache directory mismatch"
  },
  "inventory.status_5": {
    "zh": "缓存路径超出登记目录",
    "en": "Cache path is outside the registered folder"
  },
  "inventory.status_6": {
    "zh": "缓存超出条目预算",
    "en": "Cache exceeds its entry budget"
  },
  "inventory.status_7": {
    "zh": "读取权限或目录已变化",
    "en": "Read permission or folder changed"
  },
  "inventory.status_8": {
    "zh": "读取权限或目录已变化；可移除记录",
    "en": "Read permission or folder changed; you can remove this record"
  },
  "inventory.status_9": {
    "zh": "目录不可访问或身份已变化；请重新登记",
    "en": "Folder is inaccessible or its identity changed; register it again"
  },
  "inventory.status_10": {
    "zh": "索引缓存必须位于扫描目录之外，请选择更小的目录",
    "en": "Index cache must be outside the scanned folder. Choose a smaller folder"
  },
  "inventory.status_11": {
    "zh": "该目录与已有加速目录重叠，请先移除重叠项",
    "en": "This folder overlaps an accelerated folder. Remove the overlapping record first"
  },
  "inventory.status_12": {
    "zh": "目录记录已移除",
    "en": "Folder record removed"
  },
  "inventory.status_13": {
    "zh": "目录身份已变化，请移除后重新登记",
    "en": "Folder identity changed. Remove it and register again"
  },
  "inventory.status_14": {
    "zh": "后台记录中；此范围暂用现场扫描",
    "en": "Recording in the background; this scope temporarily uses live scanning"
  },
  "inventory.status_15": {
    "zh": "后台更新中；此范围暂用现场扫描",
    "en": "Updating in the background; this scope temporarily uses live scanning"
  },
  "inventory.status_16": {
    "zh": "自动更新已暂停，请缩小目录后手动更新",
    "en": "Automatic updates paused. Narrow the folder scope, then update manually"
  },
  "inventory.status_17": {
    "zh": "稍后退避重试",
    "en": "Will retry after a delay"
  },
  "inventory.status_18": {
    "zh": "目录身份变化",
    "en": "Folder identity changed"
  },
  "inventory.status_19": {
    "zh": "缓存不得位于扫描目录内",
    "en": "Cache must not be inside the scanned folder"
  },
  "inventory.status_20": {
    "zh": "目录超过 32 层，记录不完整，请缩小目录",
    "en": "Folder exceeds 32 levels; records are incomplete. Narrow the scope"
  },
  "inventory.status_21": {
    "zh": "存在不支持的特殊文件",
    "en": "Unsupported special files found"
  },
  "inventory.status_22": {
    "zh": "存在非 Unicode 文件名",
    "en": "Non-Unicode file names found"
  },
  "inventory.status_23": {
    "zh": "路径编码不支持",
    "en": "Path encoding is unsupported"
  },
  "inventory.status_24": {
    "zh": "扫描期间目录身份变化",
    "en": "Folder identity changed during the scan"
  },
  "inventory.status_25": {
    "zh": "目录已移除",
    "en": "Folder removed"
  },
  "inventory.status_26": {
    "zh": "已停止自动更新；旧缓存保留，当前使用现场扫描。立即更新或下次启动恢复",
    "en": "Automatic updates stopped; old cache preserved. Search uses live scanning. Update now or resume next startup"
  },
  "inventory.status_27": {
    "zh": "现场扫描：加速目录状态暂不可用。",
    "en": "Live scan: accelerated folder status is temporarily unavailable."
  },
  "inventory.status_28": {
    "zh": "现场扫描：读取范围或目录访问权限已变化。",
    "en": "Live scan: read scope or folder access permissions changed."
  },
  "inventory.status_29": {
    "zh": "现场扫描：登记目录当前无法读取。",
    "en": "Live scan: the registered folder cannot currently be read."
  },
  "inventory.status_30": {
    "zh": "现场扫描：部分查询目录尚未登记加速，未缩小搜索范围。",
    "en": "Live scan: some query folders are not registered for acceleration; the search scope was not narrowed."
  },
  "inventory.status_31": {
    "zh": "现场扫描：登记目录当前不在读取权限内。",
    "en": "Live scan: the registered folder is outside current read permission."
  },
  "inventory.status_32": {
    "zh": "现场扫描：登记目录路径已变化。",
    "en": "Live scan: the registered folder path changed."
  },
  "inventory.status_33": {
    "zh": "现场扫描：无法检查登记目录身份。",
    "en": "Live scan: registered folder identity could not be checked."
  },
  "inventory.status_34": {
    "zh": "现场扫描：尚无已提交快照。",
    "en": "Live scan: no committed snapshot is available yet."
  },
  "status.details_unavailable": {
    "zh": "当前本地状态详情尚不可用；请查看诊断报告或重试，不能据此确认覆盖完整或操作成功。",
    "en": "Local status details are unavailable in English. Check the diagnostic report or retry; full coverage or successful completion cannot be confirmed from this notice."
  },
  "inventory.cache_damaged": {
    "zh": "缓存损坏；请立即更新重建。",
    "en": "Cache is damaged. Update now to rebuild it."
  }
});
Object.assign(messages,backendMessages);
Object.assign(messages,{"process.outcome": {"zh": "已退出 {ended}/{total}，仍在结束 {pending}。{detail}", "en": "Exited {ended}/{total}; still waiting: {pending}. {detail}"}, "process.result": {"zh": "{name} / PID {pid}：{result}", "en": "{name} / PID {pid}: {result}"}, "inventory.update_failed": {"zh": "更新失败，旧记录保留；现场扫描。请稍后重试或缩小目录后手动更新。", "en": "Update failed; old records are preserved and search uses live scanning. Retry later or narrow the folder before updating manually."}, "inventory.load_failed": {"zh": "目录记录无法加载，原文件保留；请检查配置目录。", "en": "Folder records could not be loaded; the original file is preserved. Check the configuration directory."}});
let uiLanguage = 'en';
const missingMessageKeys = new Set();
function t(key,values={}) { const row=messages[key]; if(!row) {missingMessageKeys.add(key);return uiLanguage==='zh-CN'?'文案不可用':'Message unavailable';} return row[uiLanguage==='zh-CN'?'zh':'en'].replace(/\{([a-zA-Z0-9_]+)\}/g,(full,name)=>Object.hasOwn(values,name)?String(values[name]):full); }
const builtinMessageKeys = new Map(Object.entries(messages).flatMap(([key,value])=>[[value.zh,key],[value.en,key]]));
function builtinText(value) {const key=builtinMessageKeys.get(String(value));return key?t(key):String(value ?? '');}
function localDate(value, timeOnly=false) { const date=new Date(value); return timeOnly?date.toLocaleTimeString(uiLanguage):date.toLocaleString(uiLanguage); }
function applyLanguage(tag) { uiLanguage=tag==='zh-CN'?'zh-CN':'en'; document.documentElement.lang=uiLanguage; for(const el of document.querySelectorAll('[data-i18n]')) el.textContent=t(el.dataset.i18n);for(const attr of ['aria-label','title','placeholder','data-prompt']) for(const el of document.querySelectorAll('[data-i18n-'+attr+']')) el.setAttribute(attr,t(el.getAttribute('data-i18n-'+attr))); document.dispatchEvent(new CustomEvent('languagechange')); }
let languageDetectionFailed=false;
const languageReady = (async()=>{let tag='en';try {if(window.__TAURI__?.core) tag=(await window.__TAURI__.core.invoke('language_info')).effective_language;}catch { languageDetectionFailed=true; }applyLanguage(tag);document.documentElement.classList.remove('language-loading');})();

// These adapters accept only explicit backend-owned status/error fields, never names or chat history.
function localBuiltin(value) {
  if(value == null)return '';
  const text=String(value), key=builtinMessageKeys.get(text);
  if(key)return t(key);
  if(uiLanguage==='zh-CN' || !/[\u3400-\u9fff]/.test(text))return text;
  if(text.startsWith('正在查找文件（关键词：') && text.endsWith('）……'))return 'Finding files (keyword: '+text.slice(11,-3)+')…';
  if(text.startsWith('更新失败，旧记录保留；现场扫描。'))return t('inventory.update_failed');
  if(text.startsWith('目录记录无法加载：'))return t('inventory.load_failed');
  if(text.startsWith('缓存损坏；请立即更新重建。'))return t('inventory.cache_damaged');
  if(text.startsWith('能力未完成：'))return errorText(text.slice(6));
  return t('status.details_unavailable');
}
function errorText(value) {
  const text=String(value ?? '');
  if(uiLanguage==='zh-CN')return text;
  const exact=builtinMessageKeys.get(text);if(exact)return t(exact);
  if(text.startsWith('目录记录无法加载：'))return t('inventory.load_failed');
  if(text.startsWith('未找到 API Key'))return 'No API key found. Enter one in Settings → Models or configure its credential environment variable.';
  const http=text.match(/^模型服务返回 HTTP (\d+)：(.+)$/);if(http)return 'Model service returned HTTP '+http[1]+': '+localBuiltin(http[2]);
  if(text.startsWith('Windows 未接受打开请求（代码 ')){const code=text.match(/代码 (\d+)/)?.[1];return 'Windows rejected the open request'+(code?' (code '+code+')':'')+'. The file was not reported as displayed or running.';}
  const prefixes=[['路径不在已配置的读取范围：','Path is outside the authorized read scope: '],['路径不在已配置的写入范围：','Path is outside the authorized write scope: '],['不跟随链接或重解析点：','Links and reparse points are not followed: '],['不跟随重解析点：','Reparse points are not followed: '],['系统目录选择失败：','Windows folder selection failed: '],['Windows 未接受操作：','Windows rejected the operation: '],['无法访问 ','Cannot access ']];
  for(const [prefix,en] of prefixes)if(text.startsWith(prefix))return en+text.slice(prefix.length);
  for(const [key,row] of Object.entries(backendMessages)) if(text.startsWith(row.zh+':') || text.startsWith(row.zh+'：')) return row.en+(/^[\x00-\x7f]*$/.test(text.slice(row.zh.length))?text.slice(row.zh.length):'');
  if(!/[\u3400-\u9fff]/.test(text))return t('error.detail',{detail:text});
  return t('error.unknown');
}
