<script setup lang="ts">
    import { ref, reactive } from 'vue';
	import { ArrowRight } from '@element-plus/icons-vue';
	import { useOptionsStore } from '../stores/options';
	import { useFacesStore } from '../stores/faces';
	import { useUnlockLog } from '../hook/useUnlockLog';
	import { ElMessage } from 'element-plus';

	const optionsStore = useOptionsStore();
	const facesStore = useFacesStore();
	const { queryTodayLogs } = useUnlockLog();

	const statistics = reactive([
		{ label: '库内面容样本', value: facesStore.faceList.length, total: '无限', unit: '个', icon: 'User', color: '#409EFF', shadow: 'rgba(64, 158, 255, 0.2)' },
		{ label: '今日登录成功', value: '-', unit: '次', icon: 'CircleCheck', color: '#67C23A', shadow: 'rgba(103, 194, 58, 0.2)' },
		{ label: '今日拦截面容', value: '-', unit: '次', icon: 'Warning', color: '#F56C6C', shadow: 'rgba(245, 108, 108, 0.2)' }
	]);

	const systemStatus = ref([
		{ name: 'WinLogon 核心组件', desc: '系统登录凭据对接', active: true },
		{ name: '生物识别传感器', desc: '未知 前往设置页面设置', active: false },
		{ name: '人脸识别模型', desc: 'OpenCV', active: true }
	]);

	const recentLogs = ref([]);

	queryTodayLogs().then((result)=>{
		let successCount = 0;
		let filedCount = 0;
		result.forEach(item => {
			if(item.is_unlock == 1){
				successCount++;
			}else{
				filedCount++;
			}

			recentLogs.value.push({
				time: item.lastTime,
				user: facesStore.getFaceAliasById(item.face_id),
				action: item.is_unlock == 1 ? '面容验证通过' : '未知面容',
				status: item.is_unlock == 1 ? 'success' : 'error'
			})

			statistics[1].value = successCount;
			statistics[2].value = filedCount;
		})
	}).catch((error)=>{
		ElMessage.warning(error);
	})

	let tempCameraList = optionsStore.getOptionValueByKey('cameraList');
	let tempCameraIndex = optionsStore.getOptionValueByKey('camera');
	if(tempCameraList && tempCameraIndex){
		let tempList = JSON.parse(tempCameraList);
		systemStatus.value[1].desc = tempList[tempCameraIndex].camera_name;
		systemStatus.value[1].active = true;
	}
</script>
    
<template>
	<div class="dashboard-wrapper">
		<header class="page-header">
			<div class="header-left">
				<p>欢迎回来，管理员。系统当前运行状态：<span class="status-tag">安全</span></p>
			</div>
			<div class="header-right">
				<el-button type="primary" icon="Camera" size="large" @click="$router.push('/faces/add')">
					录入新面容
				</el-button>
			</div>
		</header>

		<el-row :gutter="24" class="stat-section">
			<el-col :span="8" v-for="(item, index) in statistics" :key="index">
				<div class="glass-card stat-item-card">
					<div class="item-icon" :style="{ color: item.color, background: item.shadow }">
						<el-icon :size="28">
							<component :is="item.icon" />
						</el-icon>
					</div>
					<div class="item-content">
						<span class="item-label">{{ item.label }}</span>
						<div class="item-data">
							<span class="item-value">{{ item.value }}</span>
							<span class="item-unit" v-if="item.total">/ {{ item.total }} {{ item.unit }}</span>
							<span class="item-unit" v-else>{{ item.unit }}</span>
						</div>
					</div>
				</div>
			</el-col>
		</el-row>

		<el-row :gutter="24" class="content-section">
			<el-col :span="10" style="height:100%;">
				<div class="glass-card main-card">
					<div class="card-title">
						<span>服务监控中心</span>
					</div>

					<div class="status-grid">
						<div v-for="s in systemStatus" :key="s.name" class="status-row">
							<div class="status-info">
								<p class="status-name">{{ s.name }}</p>
								<p class="status-desc">{{ s.desc }}</p>
							</div>
							<div class="status-indicator">
								<span :class="['dot', s.active ? 'active' : '']"></span>
								<span class="status-text">{{ s.active ? '就绪' : '故障' }}</span>
							</div>
						</div>
					</div>
				</div>
			</el-col>

			<el-col :span="14" style="height:100%;">
				<div class="glass-card main-card">
					<div class="card-title">
						<span>今日登录统计</span>
						<el-button link @click="$router.push('/logs')">
							完整日志 <el-icon class="el-icon--right">
								<ArrowRight />
							</el-icon>
						</el-button>
					</div>

					<div class="timeline-container">
						<el-timeline>
							<el-timeline-item v-for="(log, index) in recentLogs" :key="index"
								:type="log.status === 'success' ? 'primary' : 'danger'" :timestamp="log.time"
								size="large">
								<div class="log-entry" :class="log.status">
									<span class="log-user">{{ log.user }}</span>
									<span class="log-action">{{ log.action }}</span>
								</div>
							</el-timeline-item>
						</el-timeline>
					</div>
				</div>
			</el-col>
		</el-row>
	</div>
</template>
    
<style scoped>
	.dashboard-wrapper {
		display: flex;
		flex-direction: column;
		height: 100%;
		color: #2c3e50;
		animation: pageEnter 0.6s cubic-bezier(0.22, 1, 0.36, 1);
	}

	@keyframes pageEnter {
		from {
			opacity: 0;
			transform: translateY(20px);
		}

		to {
			opacity: 1;
			transform: translateY(0);
		}
	}

	.glass-card {
		background: #ffffff;
		border: 1px solid #eaeefb;
		border-radius: 16px;
		padding: 24px;
		box-shadow: 0 4px 12px rgba(0, 0, 0, 0.03);
		transition: all 0.3s ease;
	}

	.glass-card:hover {
		box-shadow: 0 12px 24px rgba(0, 0, 0, 0.06);
	}

	/* 头部样式 */
	.page-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		margin-bottom: 32px;
	}

	.page-header h1 {
		font-size: 28px;
		font-weight: 700;
		margin: 0 0 8px 0;
		letter-spacing: -0.5px;
	}

	.status-tag {
		color: #67C23A;
		font-weight: bold;
		background: #f0f9eb;
		padding: 2px 8px;
		border-radius: 4px;
	}

	/* 统计卡片样式 */
	.stat-section {
		margin-bottom: 24px;
	}

	.stat-item-card {
		display: flex;
		align-items: center;
		gap: 20px;
	}

	.item-icon {
		width: 60px;
		height: 60px;
		border-radius: 14px;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.item-label {
		display: block;
		font-size: 14px;
		color: #909399;
		margin-bottom: 4px;
	}

	.item-value {
		font-size: 26px;
		font-weight: 700;
		color: #1f2f3d;
	}

	.item-unit {
		font-size: 14px;
		color: #909399;
		margin-left: 6px;
	}

	/* 主内容区域卡片 */
	.content-section{
		flex-grow: 1;
		height: 0px;
	}

	.main-card {
		height: calc(100% - 80px);
		display: flex;
		flex-direction: column;
	}

	.card-title {
		display: flex;
		justify-content: space-between;
		align-items: center;
		font-size: 17px;
		font-weight: 600;
		margin-bottom: 20px;
		padding-bottom: 15px;
		border-bottom: 1px solid #f2f6fc;
	}

	/* 服务状态列表样式 */
	.status-grid {
		flex-grow: 1;
	}

	.status-row {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 12px 0;
	}

	.status-name {
		font-size: 14px;
		font-weight: 500;
		margin: 0;
	}

	.status-desc {
		font-size: 12px;
		color: #a8abb2;
		margin: 4px 0 0 0;
	}

	.status-indicator {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.dot {
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: #f56c6c;
		position: relative;
	}

	.dot.active {
		background: #67c23a;
		box-shadow: 0 0 8px #67c23a;
	}

	.status-text {
		font-size: 13px;
		font-weight: 600;
		margin-left: 5px;
	}

	/* 时间轴日志样式 */
	.timeline-container {
		padding-left: 10px;
		overflow-y: auto;
	}

	.log-entry {
		padding: 10px 15px;
		border-radius: 8px;
		background: #f8f9fb;
		display: flex;
		gap: 15px;
	}

	.log-entry.error {
		background: #fff5f5;
	}

	.log-user {
		font-weight: 600;
		color: #409EFF;
		min-width: 60px;
	}

	.log-action {
		color: #606266;
		font-size: 13px;
	}
</style>