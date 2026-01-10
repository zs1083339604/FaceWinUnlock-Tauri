<script setup lang="ts">
	import { ref, computed, watch, onMounted } from 'vue'
	import {
		Search,
		Warning,
		InfoFilled,
		CircleCloseFilled
	} from '@element-plus/icons-vue'
	import { useFile } from '../hook/useFile'
	import { useUnlockLog } from '../hook/useUnlockLog';
	import { ElMessage } from 'element-plus';

	const logsType = ref('unlock');
	const searchQuery = ref('');
	const filterLevel = ref('All');
	const { readText } = useFile();
	const { queryLogsByPage } = useUnlockLog();

	// 分页相关配置
	const currentPage = ref(1);
	const pageSize = ref(10);
	const total = ref(0);
	const loading = ref(false);

	const logs = ref([]);

	// 解析登录日志
	const parseUnlockLogs = (data) => {
		if (!Array.isArray(data)) return [];
		return data.map(item => ({
			createTime: item.lastTime,
			level: 'INFO',
			module: '登录',
			content: item.is_unlock === 1 ? '登录成功' : '登录失败'
		}));
	};

	const formatModuleName = (module) => {
		if (!module) return '未知模块';
		
		// [webview:<anonymous>@http://localhost:1420/src/App.vue:45:3] 取最后/后的内容
		if (module.includes('webview:') && module.includes('/')) {
			const lastSlashIndex = module.lastIndexOf('/');
			return module.slice(lastSlashIndex + 1);
		}
		
		// [facewinunlock_tauri_lib::utils::api] 取::最后一个部分
		if (module.includes('::')) {
			const lastColonIndex = module.lastIndexOf('::');
			return module.slice(lastColonIndex + 2);
		}
		
		// 其他格式直接返回原模块名
		return module;
	};
	
	// 解析程序日志
	const parseSoftLogs = (logText) => {
		if (!logText) return [];
		// 按行分割并过滤空行
		const lines = logText.split('\n').filter(line => line.trim());
		return lines.map(line => {
			// 正则匹配格式：[日期][时间][级别][模块] 内容
			const reg = /\[(\d{4}-\d{2}-\d{2})\]\[(\d{2}:\d{2}:\d{2})\]\[(\w+)\]\[(.+?)\]\s+(.*)/;
			const match = line.match(reg);
			if (match) {
				const rawModule = match[4]; // 原始模块名
				const formattedModule = formatModuleName(rawModule); // 格式化模块名

				return {
					createTime: `${match[1]} ${match[2]}`, // 拼接完整时间
					level: match[3], // 日志级别（DEBUG/INFO/WARN/ERROR）
					module: formattedModule, // 模块名
					content: match[5] || ''
				};
			}
			// 匹配失败时的兜底处理
			return {
				createTime: new Date().toLocaleString(),
				level: 'INFO',
				module: '未知模块',
				content: line
			};
		});
	};

	// 解析DLL日志
	const parseDllLogs = (logText) => {
		if (!logText) return [];
		const lines = logText.split('\n').filter(line => line.trim());
		return lines.map(line => {
			// 正则匹配：时间 [级别] 内容
			const reg = /^(\d{2}:\d{2}:\d{2})\s+\[(\w+)\]\s+(.*)/;
			const match = line.match(reg);
			if (match) {
				return {
					createTime: `${match[1]}`, // 补充日期+时间
					level: match[2], // 日志级别
					module: 'DLL', // 固定模块名
					content: match[3] || ''
				};
			}
			// 匹配失败兜底
			return {
				createTime: `${line.slice(0, 8)}`,
				level: 'INFO',
				module: 'DLL',
				content: line
			};
		});
	};

	// 根据logsType获取日志
	const fetchLogs = async () => {
		loading.value = true;
		try {
			let logData = [];
			if (logsType.value === 'unlock') {
				// 分页查询数据库
				const res = await queryLogsByPage(currentPage.value, pageSize.value);
				total.value = res.total || 0;
				logData = parseUnlockLogs(res.list || []);
			} else if (logsType.value === 'soft') {
				// 读取文件并解析
				const res = await readText('logs/app.log');
				const allLogs = parseSoftLogs(res);
				total.value = allLogs.length;
				logData = allLogs;
			} else if (logsType.value === 'dll') {
				// 读取文件并解析
				const res = await readText('logs/facewinunlock.log');
				const allLogs = parseDllLogs(res);
				total.value = allLogs.length;
				logData = allLogs;
			}
			logs.value = logData;
		} catch (error) {
			ElMessage.error(`获取日志失败：${error.message}`);
			logs.value = [];
			total.value = 0;
		} finally {
			loading.value = false;
		}
	};

	// 监听日志类型/分页参数变化，重新获取日志
	watch([logsType, currentPage, pageSize, filterLevel, searchQuery], fetchLogs, { immediate: true });

	// 页面挂载时初始化加载日志
	onMounted(() => {
		fetchLogs();
	});

	// 先计算所有符合筛选条件的日志（不包含分页）
	const allFilteredLogs = computed(() => {
		return logs.value.filter(l => {
			const matchLevel = filterLevel.value === 'All' || l.level === filterLevel.value;
			const matchSearch = l.content.toLowerCase().includes(searchQuery.value.toLowerCase()) ||
			l.module.toLowerCase().includes(searchQuery.value.toLowerCase());
			return matchLevel && matchSearch;
		});
	})

	const filteredLogs = computed(() => {
		if (logsType.value === 'unlock') {
			return allFilteredLogs.value;
		} else {
			total.value = allFilteredLogs.value.length;
			const start = (currentPage.value - 1) * pageSize.value;
			const end = start + pageSize.value;
			return allFilteredLogs.value.slice(start, end);
		}
	});

	const getLevelStatus = (level) => {
		const map = {
			INFO: { type: 'info', icon: InfoFilled, color: '#909399' },
			DEBUG: { type: 'primary', icon: InfoFilled, color: '#409EFF' },
			WARN: { type: 'warning', icon: Warning, color: '#E6A23C' },
			ERROR: { type: 'danger', icon: CircleCloseFilled, color: '#F56C6C' }
		}
		return map[level] || { 
			type: 'info', 
			icon: InfoFilled, 
			color: '#606266' 
		}
	}
</script>
    
<template>
	<div class="options-container">
		<div class="settings-card">
			<div class="filter-toolbar">
				<el-radio-group v-model="logsType" size="default" class="level-filter">
					<el-radio-button label="登录日志" value="unlock" />
					<el-radio-button label="程序日志" value="soft" />
					<el-radio-button label="DLL日志" value="dll" />
				</el-radio-group>
				<div class="filter-right">
					<el-select v-model="filterLevel" placeholder="日志级别" style="width: 240px; margin-right: 10px;">
						<el-option :label="'全部'" :value="'All'" />
						<el-option :label="'信息'" :value="'INFO'" />
						<el-option :label="'调试'" :value="'DEBUG'" />
						<el-option :label="'警告'" :value="'WARN'" />
						<el-option :label="'错误'" :value="'ERROR'" />
					</el-select>
					<el-input v-model="searchQuery" placeholder="关键词搜索 (模块、内容...)" class="search-input"
						:prefix-icon="Search" clearable />
				</div>
			</div>

			<div class="table-container">
				<el-table :data="filteredLogs" style="width: 100%" height="100%" :row-class-name="({row}) => 'row-' + row.level.toLowerCase()"  v-loading="loading">
					<el-table-column prop="createTime" label="时间戳" width="180">
						<template #default="{ row }">
							<span class="time-col">{{ row.createTime }}</span>
						</template>
					</el-table-column>

					<el-table-column label="级别" width="120">
						<template #default="{ row }">
							<div class="level-cell">
								<el-icon :color="getLevelStatus(row.level).color">
									<component :is="getLevelStatus(row.level).icon" />
								</el-icon>
								<span :class="['level-text', row.level.toLowerCase()]">{{ row.level }}</span>
							</div>
						</template>
					</el-table-column>

					<el-table-column prop="module" label="模块" width="120">
						<template #default="{ row }">
							<el-tag size="small" effect="plain" type="info">[{{ row.module }}]</el-tag>
						</template>
					</el-table-column>

					<el-table-column prop="content" label="详情内容">
						<template #default="{ row }">
							<code class="content-code">{{ row.content }}</code>
						</template>
					</el-table-column>
				</el-table>
			</div>

			<div class="logs-footer">
				<span>当前筛选条件下共 <b>{{ filteredLogs.length }}</b> 条记录</span>
				<el-pagination
					@size-change="val => pageSize = val"
					@current-change="val => currentPage = val"
					:current-page="currentPage"
					:page-sizes="[10, 20, 50, 100]"
					:page-size="pageSize"
					layout="total, sizes, prev, pager, next"
					:total="total"
					style="text-align: right;"
				>
				</el-pagination>
			</div>
		</div>
	</div>
</template>
    
<style scoped>
    .options-container {
		height: 100%;
	}

	.settings-card {
		background: #fff;
		border-radius: 12px;
		box-shadow: 0 4px 16px rgba(0,0,0,0.04);
		border: 1px solid #e4e7ed;
		overflow: hidden;
		margin: 0 auto;
		height: 100%;
		display: flex;
		flex-direction: column;
	}

	/* 过滤工具栏 */
	.filter-toolbar {
		padding: 16px 24px;
		background: #fafafa;
		display: flex;
		justify-content: space-between;
		align-items: center;
		border-bottom: 1px solid #f2f6fc;
	}

	.search-input {
		width: 280px;
	}

	/* 表格样式 */
	.table-container {
		flex: 1;
		padding: 0 10px;
		overflow: hidden;
	}

	/* 自定义表格列样式 */
	.time-col {
		font-family: 'Courier New', Courier, monospace;
		font-size: 13px;
		color: #606266;
	}

	.level-cell {
		display: flex;
		align-items: center;
		gap: 6px;
	}

	.level-text {
		font-weight: 700;
		font-size: 12px;
	}

	.level-text.info {
		color: #909399;
	}

	.level-text.warning {
		color: #E6A23C;
	}

	.level-text.danger {
		color: #F56C6C;
	}

	.content-code {
		font-family: 'Consolas', monospace;
		font-size: 13px;
		color: #333;
	}

	/* 底部状态栏 */
	.logs-footer {
		padding: 12px 24px;
		background: #fff;
		border-top: 1px solid #f2f6fc;
		font-size: 13px;
		color: #909399;
		display: flex;
		justify-content: space-between;
		align-items: center;
	}

	/* 深度选择器修改表格 Hover 背景 */
	:deep(.el-table__row:hover) {
		cursor: pointer;
	}

	:deep(.custom-tabs .el-tabs__header) {
		margin: 0;
	}

	:deep(.custom-tabs .el-tabs__nav-wrap::after) {
		display: none;
	}
</style>