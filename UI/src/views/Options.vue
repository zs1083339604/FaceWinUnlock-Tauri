<script setup lang="ts">
	import { ref, reactive } from 'vue'
	import { ElMessage, ElMessageBox } from 'element-plus'
	import {
		Unlock,
		Operation,
		VideoCamera,
		InfoFilled,
		Refresh
	} from '@element-plus/icons-vue'
	import { useOptionsStore } from '../stores/options'
	import { invoke } from '@tauri-apps/api/core'
	import { formatObjectString } from '../utils/function'
	import { info, error as errorLog, warn } from '@tauri-apps/plugin-log';
	import { selectCustom } from '../utils/sqlite'
	import { appCacheDir } from '@tauri-apps/api/path';
	import { useRouter } from 'vue-router'
	import { openUrl } from '@tauri-apps/plugin-opener';

	// 自启判断
	invoke("check_global_autostart").then((result)=>{
		config.autoStart = result.data.enable;
	}).catch((error)=>{
		ElMessage.warning(formatObjectString("查询自启状态失败 ", error));
	});

	const optionsStore = useOptionsStore();
	const router = useRouter();

	const activeTab = ref('app')

	const cameraList = ref([]);
	const cameraListLoading = ref(false);

	const config = reactive({
		camera: optionsStore.getOptionValueByKey('camera') || "-1",
		// detectThreshold: 60,
		autoStart: true,
		faceRecogDelay: parseFloat(optionsStore.getOptionValueByKey('faceRecogDelay')) || 10.0,
	})

	const dllConfig = reactive({
		showTile: optionsStore.getOptionValueByKey('showTile') ? (optionsStore.getOptionValueByKey('showTile') == 'false' ? false : true) : true,
	})

	const refreshCameraList = ()=>{
		cameraListLoading.value = true;
		// 因为不确定之前摄像头是否还可用，强制设为-1
		config.camera = "-1";
		// 获取摄像头列表
		invoke("get_camera").then((result)=>{
			// 清空列表
			cameraList.value.length = 0;

			// 添加列表
			result.data.forEach(item => {
				if(config.camera == "-1"){
					config.camera = item.capture_index;
				}
				cameraList.value.push(item);
			});

			// 立即添加到数据库，不能等用户点
			return optionsStore.saveOptions({
				cameraList: JSON.stringify(cameraList.value),
				camera: config.camera
			});
		}).then(()=>{
			ElMessage.success("获取摄像头列表成功");
		}).catch((error)=>{
			ElMessage.error(formatObjectString(error));
		}).finally(()=>{
			cameraListLoading.value = false;
		})
	}

	// 判断是否获取过摄像头列表
	let tempCameraList = optionsStore.getOptionValueByKey('cameraList');
	if(!tempCameraList){
		refreshCameraList();
	}else{
		cameraList.value = JSON.parse(tempCameraList);
	}

	// 自启切换
	const handleAutoStartChange = ()=>{
		if(config.autoStart){
			invoke("enable_global_autostart").catch(()=>{
				config.autoStart = false;
				ElMessage.error("请确保已添加开机启动权限");
			});
		}else{
			invoke("disable_global_autostart").catch(()=>{
				config.autoStart = true;
				ElMessage.error("取消开机启动失败，请重新尝试");
			});
		}
	}

	const saveAppConfig = () => {
		optionsStore.saveOptions({
			camera: config.camera,
			faceRecogDelay: config.faceRecogDelay
		}).then((errorArray)=>{
			if(errorArray.length > 0){
				ElMessage.warning({
                    dangerouslyUseHTMLString: true,
                    message: `${result.length} 个配置保存失败: <br />${result.join("<br />")}`
                })
			}else{
				ElMessage.success("保存成功");
			}
		}).catch();
	}

	const applyDllSettings = () => {
		invoke("write_to_registry", {items: [
			{
				key: "SHOW_TILE",
				value: dllConfig.showTile ? "1" : "0"
			}
		]}).then(()=>{
			return optionsStore.saveOptions({
				showTile: dllConfig.showTile
			})
		}).then((errorArray)=>{
			if(errorArray.length > 0){
				ElMessage.warning({
                    dangerouslyUseHTMLString: true,
                    message: `${result.length} 个配置保存失败: <br />${result.join("<br />")}`
                })
			}else{
				ElMessage.success("保存成功");
			}
		}).catch((error)=>{
			const info = formatObjectString("保存DLL配置失败: ", error);
			ElMessage.error(info);
			errorLog(info);
		})
	}

	const clearCache = () => {
		ElMessageBox.confirm('这将清除数据库缓存，软件缓存请手动关闭软件后，删除打开的 EBWebView 文件夹', '注意', {
			confirmButtonText: '确定清除',
			cancelButtonText: '取消',
			type: 'warning'
		}).then(async () => {
			try {
				await selectCustom("VACUUM;");
			} catch (error) {
				const info = formatObjectString("删除数据库缓存失败: ", error);
				ElMessage.error(info);
				errorLog(info);
				return;
			}

			// 走到这，其实 EBWebView 必然是被软件占用的，所以直接rust删除必定会失败
			// 但也有一些方法，但是我懒得写了，后面看到的大佬，有想实现的，可以自己实现一下
			// 	1. 用win32 Api单独写一个程序，点到这里唤醒程序，等本程序退出后，清除缓存
			// 	2. 用win32包裹此程序启动，启动时先启动win32的程序，判断缓存目录是否有清除标记，如果有就清除缓存，并启动本软件，如果没有直接启动本软件
			// 	   当走到这里时，给缓存目录添加标记，等待下一次开启自动清除
			ElMessageBox.alert('数据库缓存已清除，即将打开软件缓存目录，请在关闭软件后，删除 EBWebView 文件夹', '提示', {
				confirmButtonText: '确定',
				callback: () => {
					appCacheDir().then((result)=>{
						return invoke("open_directory", {path: result})
					}).catch((error)=>{
						const info = formatObjectString("打开文件夹失败: ", error);
						ElMessage.error(info);
						errorLog(info);
					})
				},
			})
		})
	}

	const uninstallDll = () => {
		ElMessageBox.confirm(
			'卸载 DLL 并还原注册表将导致无法在登录界面使用面容解锁。程序将强制回到初始化页面。', 
			'危险操作', 
			{
				confirmButtonText: '确定卸载',
				confirmButtonClass: 'el-button--danger',
				cancelButtonText: '取消',
				type: 'error'
			}
		).then(() => {
			invoke("uninstall_init").then(()=>{
				return optionsStore.saveOptions({is_initialized: 'false'});
			}).then((errorList)=>{
				if (errorList.length > 0) {
					ElMessageBox.alert(formatObjectString(errorList), '保存设置失败', {
						confirmButtonText: '确定'
					});
				} else {
					ElMessage.success('组件已卸载，并撤回了软件对注册表的操作！');
					router.push('/init');
				}
			}).catch((error)=>{
				const info = formatObjectString("卸载组件失败：", error);
				ElMessage.error(info);
				errorLog(info);
			})
		})
	}
</script>

<template>
	<div class="options-container">
		<div class="settings-card">
			<div class="settings-header">
				<div class="custom-nav">
					<div class="nav-item" :class="{ active: activeTab === 'app' }" @click="activeTab = 'app'">
						<el-icon>
							<Operation />
						</el-icon>
						软件配置
					</div>
					<div class="nav-item" :class="{ active: activeTab === 'dll' }" @click="activeTab = 'dll'">
						<el-icon>
							<Unlock />
						</el-icon>
						系统集成 (DLL)
					</div>
				</div>
				<div>
					<el-button type="primary" size="large" icon="Cpu"
						@click="activeTab === 'app' ? saveAppConfig() : applyDllSettings()">
						{{ activeTab === 'app' ? '保存本地配置' : '同步至系统注册表' }}
					</el-button>
					<el-button type="info" plain @click="openUrl('https://github.com/zs1083339604/FaceWinUnlock-Tauri')">Github</el-button>
					<el-button type="danger" plain @click="openUrl('https://gitee.com/lieranhuasha/face-win-unlock-tauri')">Gitee</el-button>
				</div>
				
			</div>

			<div class="options-content">
				<div v-if="activeTab === 'app'" class="fade-in">
					<el-row :gutter="40">
						<el-col :span="14">
							<section class="config-group">
								<h4 class="group-title">识别参数</h4>
								<el-form label-position="top">
									<el-form-item label="默认采集设备">
										<div class="select-with-refresh">
											<el-select v-model="config.camera" style="width: 100%">
												<template #prefix>
													<el-icon>
														<VideoCamera />
													</el-icon>
												</template>
												<el-option v-for="item in cameraList" :key="item.capture_index" :value="item.capture_index" :label="item.camera_name" :disabled="!item.is_valid"/>
											</el-select>
											<el-button 
												:icon="Refresh" 
												class="refresh-camera-btn"
												title="刷新采集设备列表"
												:loading="cameraListLoading"
												@click="refreshCameraList"
											/>
										</div>
									</el-form-item>

									<!-- cy: 人脸的置信度还是放添加页面更好 -->
									<!-- <el-form-item label="人脸检测置信度">
										<div class="slider-info">
											<span class="val">{{ config.detectThreshold }}%</span>
											<span class="desc">建议 60%，数值越高越安全</span>
										</div>
										<el-slider v-model="config.detectThreshold" :min="10" :max="100" />
									</el-form-item> -->
								</el-form>
							</section>

							<section class="config-group">
								<h4 class="group-title">通用行为</h4>
								<div class="option-row">
									<div class="row-text">
										<p class="label">随 Windows 自动启动 *</p>
										<p class="sub">登录系统后自动激活后台识别引擎（不用点保存）</p>
									</div>
									<el-switch v-model="config.autoStart" @change="handleAutoStartChange"/>
								</div>
								<!-- cy: 本来想设置锁屏界面有操作后调用，但锁屏界面是隔离的，hook不生效，也未找到有效解决方案，先用这个 -->
								<div class="option-row">
									<div class="row-text">
										<p class="label">锁屏后面容识别延迟（秒）</p>
										<p class="sub">锁屏完成后，延迟指定秒数调用摄像头进行面容识别</p>
									</div>
									<el-input-number 
										v-model="config.faceRecogDelay"
										:min="0.1" 
										:max="120" 
										:step="1" 
										:precision="1"
										style="width: 120px;"
									/>
								</div>
							</section>
						</el-col>

						<el-col :span="10">
							<section class="config-group danger-zone">
								<h4 class="group-title red-text">维护与卸载</h4>
								<div class="danger-box">
									<div class="danger-item">
										<span>清除数据库和软件缓存</span>
										<el-button type="warning" size="small" plain @click="clearCache">点击清除</el-button>
									</div>
									<el-divider />
									<div class="danger-item">
										<span>重新初始化</span>
										<el-button type="warning" size="small" plain @click="$router.push('/init')">点击初始化</el-button>
									</div>
									<p class="danger-footer">
										<el-icon>
											<InfoFilled />
										</el-icon> 初始化需要管理员权限
									</p>
									<el-divider />
									<div class="danger-item">
										<span>卸载WinLogon解锁组件</span>
										<el-button type="danger" size="small" @click="uninstallDll">点击卸载</el-button>
									</div>
									<p class="danger-footer">
										<el-icon>
											<InfoFilled />
										</el-icon> 卸载操作需要管理员权限
									</p>
								</div>
							</section>
						</el-col>
					</el-row>
				</div>

				<div v-if="activeTab === 'dll'" class="fade-in">
					<el-alert title="系统级配置修改" type="info" description="以下选项通过 Rust 后端同步至 Windows 注册表，修改后需要重新锁定计算机生效。"
						show-icon :closable="false" />

					<div class="dll-settings">
						<div class="option-row">
							<div class="row-text">
								<p class="label">启用登录界面磁贴 (Tile)</p>
								<p class="sub">在 Windows 锁屏界面显示解锁磁贴</p>
							</div>
							<el-switch v-model="dllConfig.showTile" />
						</div>
					</div>
				</div>
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
	}

	.settings-header {
		padding: 0 30px;
		height: 70px;
		display: flex;
		justify-content: space-between;
		align-items: center;
		border-bottom: 1px solid #f2f6fc;
	}

	.custom-nav {
		display: flex;
		background: #f0f2f5;
		padding: 4px;
		border-radius: 8px;
		gap: 4px;
	}

	.nav-item {
		padding: 8px 20px;
		border-radius: 6px;
		font-size: 14px;
		cursor: pointer;
		display: flex;
		align-items: center;
		gap: 8px;
		transition: all 0.2s;
		color: #606266;
	}

	.nav-item:hover {
		color: #409EFF;
	}

	.nav-item.active {
		background: #fff;
		color: #409EFF;
		box-shadow: 0 2px 6px rgba(0,0,0,0.08);
		font-weight: 600;
	}

	.options-content {
		padding: 0px 30px;
		min-height: 450px;
	}

	.group-title {
		font-size: 15px;
		font-weight: 600;
		margin-bottom: 10px;
		color: #303133;
		display: flex;
		align-items: center;
	}

	.select-with-refresh {
		position: relative;
		width: 100%;
		display: flex;
		align-items: center;
	}

	.refresh-camera-btn{
		margin-left: 10px;
	}

	.config-group {
		margin-bottom: 35px;
	}

	.option-row {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 16px 0;
		border-bottom: 1px solid #f2f6fc;
	}

	.row-text .label {
		font-size: 14px;
		font-weight: 500;
		margin: 0;
		color: #2c3e50;
	}

	.row-text .sub {
		font-size: 12px;
		color: #909399;
		margin: 4px 0 0 0;
	}

	.slider-info {
		display: flex;
		justify-content: space-between;
		width: 100%;
		margin-bottom: -10px;
	}

	.slider-info .val {
		color: #409EFF;
		font-weight: bold;
	}

	.slider-info .desc {
		font-size: 12px;
		color: #909399;
	}

	.danger-box {
		background: #fef0f0;
		border-radius: 10px;
		padding: 20px;
		border: 1px solid #fde2e2;
	}

	.danger-item {
		display: flex;
		justify-content: space-between;
		align-items: center;
		font-size: 13px;
		color: #606266;
	}

	.danger-footer {
		margin-top: 5px;
		font-size: 12px;
		color: #f56c6c;
		display: flex;
		align-items: center;
		gap: 5px;
	}

	.red-text {
		color: #f56c6c;
	}

	.fade-in {
		animation: fadeIn 0.3s ease-in-out;
	}

	@keyframes fadeIn {
		from { opacity: 0; transform: translateY(5px); }
		to { opacity: 1; transform: translateY(0); }
	}
</style>