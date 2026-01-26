<script setup lang="ts">
	import { ref, computed } from 'vue';
	import { RouterView, useRouter } from 'vue-router';
	import { ElMessage } from 'element-plus';
	import {
		Avatar,
		Odometer,
		User,
		Setting,
		Tools,
		VideoCamera,
		SwitchButton
	} from '@element-plus/icons-vue'
	import { useAuthStore } from '../stores/auth';
	
	const router = useRouter();
	const authStore = useAuthStore();
	
	const version = ref(localStorage.getItem("version") || 'unknown');
	
	// 是否启用了登录功能
	const isLoginEnabled = computed(() => authStore.isLoginEnabled);
	
	// 处理退出登录
	const handleLogout = () => {
		authStore.logout();
		ElMessage.success('已退出登录');
		router.push('/login');
	};
</script>

<template>
	 <el-container class="layout-container">
        <el-aside width="240px" class="aside-menu">
            <div class="logo-area">
                <span class="logo-text">FaceWinUnlock</span>
            </div>

            <el-menu :default-active="$route.path" router class="custom-menu">
                <el-menu-item index="/">
                    <el-icon>
                        <Odometer />
                    </el-icon>
                    <span>概览</span>
                </el-menu-item>

				<el-menu-item index="/faces">
				    <el-icon>
				        <User />
				    </el-icon>
				    <span>面容管理</span>
				</el-menu-item>

                <el-menu-item index="/options">
                    <el-icon>
                        <Setting />
                    </el-icon>
                    <span>设置</span>
                </el-menu-item>

				<el-menu-item index="/logs">
                    <el-icon>
						<Tickets />
					</el-icon>
                    <span>日志</span>
                </el-menu-item>
            </el-menu>

            <div class="aside-footer">
				<div class="footer-left">
					<div class="social-links">
						<a href="https://github.com/zs1083339604/FaceWinUnlock-Tauri" target="_blank" class="social-link" title="GitHub">
							<svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor">
								<path d="M12 0c-6.626 0-12 5.373-12 12 0 5.302 3.438 9.8 8.207 11.387.599.111.793-.261.793-.577v-2.234c-3.338.726-4.033-1.416-4.033-1.416-.546-1.387-1.333-1.756-1.333-1.756-1.089-.745.083-.729.083-.729 1.205.084 1.839 1.237 1.839 1.237 1.07 1.834 2.807 1.304 3.492.997.107-.775.418-1.305.762-1.604-2.665-.305-5.467-1.334-5.467-5.931 0-1.311.469-2.381 1.236-3.221-.124-.303-.535-1.524.117-3.176 0 0 1.008-.322 3.301 1.23.957-.266 1.983-.399 3.003-.404 1.02.005 2.047.138 3.006.404 2.291-1.552 3.297-1.23 3.297-1.23.653 1.653.242 2.874.118 3.176.77.84 1.235 1.911 1.235 3.221 0 4.609-2.807 5.624-5.479 5.921.43.372.823 1.102.823 2.222v3.293c0 .319.192.694.801.576 4.765-1.589 8.199-6.086 8.199-11.386 0-6.627-5.373-12-12-12z"/>
							</svg>
						</a>
					</div>
					<div class="version-info">
						<span class="version-label">版本</span>
						<span class="version-number">v {{ version }}</span>
					</div>
				</div>
				<!-- 写死就行了，不就绪不会显示这个 (笑  -->
                <el-tag size="small" type="success" effect="plain">服务已就绪</el-tag>
            </div>
        </el-aside>

        <el-container>
			<el-header class="global-header" height="50px">
				<div class="left-section">
					<el-button 
						v-if="$route.path !== '/'" 
						icon="ArrowLeft" 
						circle 
						size="small"
						@click="$router.back()"
						class="back-btn"
					/>
					<span class="page-title">{{ $route.meta.title || '面容识别系统' }}</span>
				</div>
				<div class="right-section">
					<!-- 退出登录按钮（仅在启用登录时显示） -->
					<el-button
						v-if="isLoginEnabled"
						type="danger"
						size="small"
						:icon="SwitchButton"
						circle
						title="退出登录"
						@click="handleLogout"
					/>
				</div>
			</el-header>
            <el-main class="main-content">
                <router-view v-slot="{ Component }">
                    <transition name="fade-transform" mode="out-in">
                        <component :is="Component" />
                    </transition>
                </router-view>
            </el-main>
        </el-container>
    </el-container>
</template>

<style scoped>
	.layout-container {
		display: flex;
		height: 100vh;
		background-color: #f9f9f9;
	}

	/* 侧边栏样式 */
	.aside-menu {
		background-color: #ffffff;
		border-right: 1px solid #e6e6e6;
		display: flex;
		flex-direction: column;
	}

	.logo-area {
		height: 80px;
		display: flex;
		align-items: center;
		padding: 0 25px;
		gap: 12px;
	}

	.logo-text {
		font-size: 24px;
		font-weight: 600;
		color: #303133;
	}

	.custom-menu {
		border-right: none;
		flex: 1;
	}

	/* 底部状态 */
	.aside-footer {
		padding: 15px 20px;
		border-top: 1px solid #f0f0f0;
		text-align: center;
		display: flex;
		flex-direction: column;
		gap: 10px;
	}

	/* 底部左侧：社交链接和版本 */
	.footer-left {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 8px;
	}

	/* 社交链接 */
	.social-links {
		display: flex;
		align-items: center;
		gap: 4px;
	}

	.social-link {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 20px;
		height: 20px;
		border-radius: 4px;
		color: #909399;
		background-color: transparent;
		transition: all 0.3s ease;
		text-decoration: none;
	}

	.social-link:hover {
		color: #409EFF;
		background-color: #ecf5ff;
	}

	.social-link.gitee:hover {
		color: #c71d23;
		background-color: #fdf2f2;
	}

	.version-info {
		display: flex;
		align-items: center;
		gap: 4px;
		font-size: 12px;
		color: #909399;
	}

	.version-label {
		color: #c0c4cc;
	}

	.version-number {
		font-weight: 500;
		color: #606266;
	}

	/* 头部样式 */
	.global-header {
		background-color: #ffffff;
		border-bottom: 1px solid #e6e6e6;
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 0 20px;
	}

	.left-section {
		display: flex;
		align-items: center;
		gap: 12px;
	}

	.back-btn {
		margin-left: 0;
	}

	.page-title {
		font-size: 16px;
		font-weight: 500;
		color: #303133;
	}

	.right-section {
		display: flex;
		align-items: center;
		gap: 12px;
	}

	/* 主内容区域 */
	.main-content {
		padding: 20px;
		background-color: #f5f7fa;
		overflow-y: auto;
	}

	/* 过渡动画 */
	.fade-transform-enter-active,
	.fade-transform-leave-active {
		transition: all 0.2s;
	}

	.fade-transform-enter-from {
		opacity: 0;
		transform: translateX(20px);
	}

	.fade-transform-leave-to {
		opacity: 0;
		transform: translateX(-20px);
	}
</style>
