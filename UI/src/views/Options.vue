<script setup lang="ts">
	import { ref, reactive, onMounted, computed } from 'vue'
	import { ElMessage, ElMessageBox } from 'element-plus'
	import {
		Unlock,
		Operation,
		Tools,
		VideoCamera,
		InfoFilled,
		Refresh,
		Lock,
		Setting,
		Loading,
		CircleCheck,
		WarningFilled,
		FolderOpened,
		Key,
		View,
		Hide
	} from '@element-plus/icons-vue'
	import { useOptionsStore } from '../stores/options'
	import { invoke } from '@tauri-apps/api/core'
	import { formatObjectString } from '../utils/function'
	import { info, error as errorLog, warn } from '@tauri-apps/plugin-log';
	import { selectCustom } from '../utils/sqlite'
	import { appCacheDir } from '@tauri-apps/api/path';
	import { useRouter } from 'vue-router'
	import { openUrl } from '@tauri-apps/plugin-opener';
	import { useAuthStore } from '../stores/auth';

	// 自启判断
	invoke("check_global_autostart").then((result)=>{
		config.autoStart = result.data.enable;
	}).catch((error)=>{
		ElMessage.warning(formatObjectString("查询自启状态失败 ", error));
	});

	const optionsStore = useOptionsStore();
	const router = useRouter();
	const authStore = useAuthStore();

	// 添加计算属性来判断是否设置了密码
	const hasPassword = computed(() => !!authStore.passwordHash);

	const activeTab = ref('app')

	const cameraList = ref([]);
	const cameraListLoading = ref(false);

	// 登录安全配置
	const securityConfig = reactive({
		loginEnabled: false,
		password: '',
		confirmPassword: '',
		showPassword: false,
		showConfirmPassword: false,
		loading: false,
		saving: false,
		expireMinutes: 0
	});

	// 登录过期时间选项
	const expireOptions = authStore.getExpireOptions();

	// 初始化安全配置
	onMounted(() => {
		securityConfig.loginEnabled = authStore.loginEnabled && !!authStore.passwordHash;
		securityConfig.expireMinutes = authStore.getExpireMinutes();
	});

	const config = reactive({
		camera: optionsStore.getOptionValueByKey('camera') || "-1",
		// detectThreshold: 60,
		autoStart: true,
		faceRecogDelay: parseFloat(optionsStore.getOptionValueByKey('faceRecogDelay')) || 10.0,
		faceRecogType: optionsStore.getOptionValueByKey('faceRecogType') || 'operation',
		silentRun: optionsStore.getOptionValueByKey('silentRun') ? (optionsStore.getOptionValueByKey('silentRun') == 'false' ? false : true) : false,
		retryDelay: parseFloat(optionsStore.getOptionValueByKey('retryDelay')) || 10.0,
	})

	const dllConfig = reactive({
		showTile: optionsStore.getOptionValueByKey('showTile') ? (optionsStore.getOptionValueByKey('showTile') == 'false' ? false : true) : true
	})

	// 活体检测配置
	const livenessConfig = reactive({
		enabled: optionsStore.getOptionValueByKey('livenessEnabled') ? (optionsStore.getOptionValueByKey('livenessEnabled') == 'false' ? false : true) : true,
		threshold: parseFloat(optionsStore.getOptionValueByKey('livenessThreshold')) || 0.75,
		modelStatus: {
			exists: false,
			path: '',
			loading: true
		}
	})

	// 获取活体检测模型状态
	const fetchLivenessStatus = () => {
		livenessConfig.modelStatus.loading = true;
		invoke("get_liveness_status").then((result: any) => {
			livenessConfig.modelStatus.exists = result.data.model_exists;
			livenessConfig.modelStatus.path = result.data.model_path;
			livenessConfig.modelStatus.loading = false;
		}).catch((error) => {
			livenessConfig.modelStatus.loading = false;
			warn(formatObjectString("获取活体检测状态失败: ", error));
		});
	}

	onMounted(() => {
		fetchLivenessStatus();
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
			invoke("enable_global_autostart").catch((e)=>{
				config.autoStart = false;
				ElMessage.error(formatObjectString(e));
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
			faceRecogDelay: config.faceRecogDelay,
			faceRecogType: config.faceRecogType,
			silentRun: config.silentRun,
			retryDelay: config.retryDelay,
		}).then((errorArray)=>{
			if(errorArray.length > 0){
				ElMessage.warning({
                    dangerouslyUseHTMLString: true,
                    message: `${errorArray.length} 个配置保存失败: <br />${errorArray.join("<br />")}`
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
                    message: `${errorArray.length} 个配置保存失败: <br />${errorArray.join("<br />")}`
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

	// 保存活体检测配置
	const saveLivenessConfig = () => {
		optionsStore.saveOptions({
			livenessEnabled: livenessConfig.enabled.toString(),
			livenessThreshold: livenessConfig.threshold.toString()
		}).then((errorArray)=>{
			if(errorArray.length > 0){
				ElMessage.warning({
                    dangerouslyUseHTMLString: true,
                    message: `${errorArray.length} 个配置保存失败: <br />${errorArray.join("<br />")}`
                })
			}else{
				ElMessage.success("保存成功");
			}
		}).catch((error)=>{
			const info = formatObjectString("保存活体检测配置失败: ", error);
			ElMessage.error(info);
			errorLog(info);
		});
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
			ElMessageBox.alert('数据库缓存已清除，即将打开软件缓存目录，请在关闭软件WebView 文件夹后，删除 EB', '提示', {
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

	// 保存登录密码设置
	const saveLoginSecurity = async () => {
		if (securityConfig.loginEnabled) {
			// 用户是否要修改密码（输入了新密码）
			const wantsToChangePassword = !!securityConfig.password;

			// 如果已设置密码且未输入新密码，只需要保存过期时间等设置
			if (authStore.passwordHash && !wantsToChangePassword) {
				securityConfig.saving = true;
				try {
					authStore.setExpireMinutes(securityConfig.expireMinutes);
					ElMessage.success('登录设置已保存');
				} catch (error) {
					const info = formatObjectString("保存登录设置失败: ", error);
					ElMessage.error(info);
					errorLog(info);
				} finally {
					securityConfig.saving = false;
				}
				return;
			}

			// 以下是需要修改密码的情况
			if (!securityConfig.password) {
				ElMessage.warning('请输入登录密码');
				return;
			}
			if (securityConfig.password.length < 4) {
				ElMessage.warning('密码长度至少4位');
				return;
			}
			if (securityConfig.password !== securityConfig.confirmPassword) {
				ElMessage.warning('两次输入的密码不一致');
				return;
			}

			securityConfig.saving = true;
			try {
				// 先验证当前密码（如果是修改密码）
				if (authStore.passwordHash && securityConfig.password) {
					const verifyResult = await invoke('verify_app_password', { password: securityConfig.password });
					if (verifyResult.code === 200) {
						ElMessage.warning('新密码不能与当前密码相同');
						securityConfig.saving = false;
						return;
					}
				}

				// 设置新密码
				const result = await invoke('hash_password_cmd', { password: securityConfig.password });
				if (result.code === 200) {
					// 保存到数据库
					await optionsStore.saveOptions({
						app_password_hash: result.data.hash
					});
					// 更新本地状态
					authStore.passwordHash = result.data.hash;
					authStore.loginEnabled = true;
					authStore.setExpireMinutes(securityConfig.expireMinutes);
					localStorage.setItem('auth_login_enabled', 'true');
					localStorage.setItem('auth_password_hash', result.data.hash);
					ElMessage.success('登录密码设置成功');
					securityConfig.password = '';
					securityConfig.confirmPassword = '';
				} else {
					ElMessage.error('密码设置失败');
				}
			} catch (error) {
				const info = formatObjectString("保存登录密码失败: ", error);
				ElMessage.error(info);
				errorLog(info);
			} finally {
				securityConfig.saving = false;
			}
		} else {
			// 禁用登录
			try {
				// 清除数据库中的密码
				// 先查询是否存在
				const existing = optionsStore.getOptionByKey('app_password_hash');
				if (existing.index !== -1) {
					await optionsStore.saveOptions({
						app_password_hash: ''
					});
				}
				// 更新本地状态
				authStore.clearPassword();
				ElMessage.success('已禁用应用登录');
			} catch (error) {
				const info = formatObjectString("禁用登录失败: ", error);
				ElMessage.error(info);
				errorLog(info);
			}
		}
	}

	// 登出当前用户
	const handleLogout = () => {
		authStore.logout();
		router.push('/login');
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
					<div class="nav-item" :class="{ active: activeTab === 'security' }" @click="activeTab = 'security'">
						<el-icon>
							<Key />
						</el-icon>
						登录安全
					</div>
					<div class="nav-item" :class="{ active: activeTab === 'liveness' }" @click="activeTab = 'liveness'">
						<el-icon>
							<Lock />
						</el-icon>
						活体检测
					</div>
					<div class="nav-item" :class="{ active: activeTab === 'dll' }" @click="activeTab = 'dll'">
						<el-icon>
							<Unlock />
						</el-icon>
						系统集成 (DLL)
					</div>
					<div class="nav-item" :class="{ active: activeTab === 'maintenance' }" @click="activeTab = 'maintenance'">
						<el-icon>
							<Tools />
						</el-icon>
						维护与卸载
					</div>
				</div>
					<div>
						<el-button v-if="activeTab !== 'maintenance' && activeTab !== 'security'" type="primary" size="large"
							@click="activeTab === 'app' ? saveAppConfig() : (activeTab === 'liveness' ? saveLivenessConfig() : applyDllSettings())">
							{{ activeTab === 'dll' ? '同步至系统注册表' : '保存' }}
						</el-button>
						<el-button v-if="activeTab === 'security'" type="primary" size="large" :loading="securityConfig.saving"
							@click="saveLoginSecurity">
							保存
						</el-button>
					</div>
				
			</div>

			<div class="options-content">
				<div v-if="activeTab === 'app'" class="fade-in">
					<el-row :gutter="40">
						<el-col :span="24">
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
								<div class="option-row">
									<div class="row-text">
										<p class="label">是否静默自启</p>
										<p class="sub">软件开机自动后，隐藏窗口界面</p>
									</div>
									<el-switch v-model="config.silentRun"/>
								</div>
								<div class="option-row" title="开发未完成，暂时不可用">
									<div class="row-text">
										<p class="label">开机面容识别</p>
										<p class="sub">第一次开机时就可以使用面容识别，开发未完成，暂时不可用</p>
									</div>
									<el-switch :value="false" :disabled="true"/>
								</div>
								<div class="option-row">
									<div class="row-text">
										<p class="label">面容识别方式</p>
										<p class="sub">锁屏完成后，用什么方式调用面容识别代码</p>
									</div>
									<el-select v-model="config.faceRecogType" style="width: 170px">
										<el-option :value="'operation'" :label="'用户操作 (支持重试)'"/>
										<el-option :value="'delay'" :label="'延迟时间'"/>
									</el-select>
								</div>
								<div class="option-row" v-if="config.faceRecogType === 'delay'">
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
								<div class="option-row" v-else>
									<div class="row-text">
										<p class="label">重试时间（秒）</p>
										<p class="sub">在面容不匹配时，时隔多长时间允许重试</p>
									</div>
									<el-input-number 
										v-model="config.retryDelay"
										:min="1" 
										:max="120" 
										:step="1" 
										:precision="1"
										style="width: 120px;"
									/>
								</div>
							</section>
						</el-col>
					</el-row>
					</div>

					<div v-if="activeTab === 'maintenance'" class="fade-in">
						<el-row :gutter="40">
							<el-col :span="24">
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
											 需要管理员权限
										</p>
										<el-divider />
										<div class="danger-item">
											<span>卸载WinLogon解锁组件</span>
											<el-button type="danger" size="small" @click="uninstallDll">点击卸载</el-button>
										</div>
										<p class="danger-footer">
											 需要管理员权限
										</p>
									</div>
								</section>
							</el-col>
						</el-row>
					</div>

					<div v-if="activeTab === 'security'" class="fade-in">
					<el-row :gutter="40">
						<el-col :span="24">
							<section class="config-group">
								<h4 class="group-title">
									<el-icon><Key /></el-icon>
									应用登录密码
								</h4>

								<!-- 启用登录开关 -->
								<div class="option-row">
									<div class="row-text">
										<p class="label">启用应用登录</p>
										<p class="sub">打开应用时需要输入密码验证，增强安全性</p>
									</div>
									<el-switch v-model="securityConfig.loginEnabled" />
								</div>

								<!-- 密码输入框（启用登录时显示） -->
								<div v-if="securityConfig.loginEnabled" class="password-form">
									<el-form label-position="top">
										<el-form-item label="设置登录密码(为空则不修改)">
											<el-input
												v-model="securityConfig.password"
												type="password"
												show-password
												placeholder="请输入密码（至少4位）"
												clearable
											/>
										</el-form-item>
										<el-form-item label="确认密码">
											<el-input
												v-model="securityConfig.confirmPassword"
												type="password"
												show-password
												placeholder="请再次输入密码"
												clearable
											/>
										</el-form-item>
									</el-form>
								</div>

								<!-- 提示信息 -->
								<div class="security-tips">
									<el-alert
										v-if="securityConfig.loginEnabled"
										title="安全提示"
										type="success"
										description="密码已启用。密码使用 bcrypt 算法加密本地存储。"
										show-icon
										:closable="false"
									/>
									<el-alert
										v-else
										title="安全提示"
										type="info"
										description="当前未设置应用登录密码，任何人都可以直接打开应用。如需增强安全性，请启用登录并设置密码。"
										show-icon
										:closable="false"
									/>
								</div>
							</section>

							<!-- 登录过期时间设置（启用登录时显示） -->
							<div v-if="securityConfig.loginEnabled" class="expire-config">
								<div class="option-row">
									<div class="row-text">
										<p class="label">登录过期时间</p>
										<p class="sub">登录状态过期后需要重新输入密码</p>
									</div>
									<el-select v-model="securityConfig.expireMinutes" style="width: 200px">
										<el-option
											v-for="option in expireOptions"
											:key="option.value"
											:label="option.label"
											:value="option.value"
										/>
									</el-select>
								</div>
							
							</div>
						</el-col>
					</el-row>
				</div>

				<div v-if="activeTab === 'dll'" class="fade-in">

					<div class="dll-settings">
						<div class="option-row">
							<div class="row-text">
								<p class="label">启用登录界面磁贴 (Tile)</p>
								<p class="sub">在 Windows 锁屏界面显示解锁磁贴</p>
							</div>
							<el-switch v-model="dllConfig.showTile" />
						</div>
					</div>
					<el-alert title="系统级配置修改" type="info" description="以上选项通过 Rust 后端同步至 Windows 注册表，修改后需要重新锁定计算机生效。"
						show-icon :closable="false" />
				</div>

				<div v-if="activeTab === 'liveness'" class="fade-in">
					<el-row :gutter="40">
						<el-col :span="24">
							<section class="config-group">
								<h4 class="group-title">
									<el-icon><Lock /></el-icon>
									活体检测配置
								</h4>

								<!-- 活体检测开关 -->
								<div class="option-row">
									<div class="row-text">
										<p class="label">启用活体检测(静默)</p>
										<p class="sub">在解锁时进行活体验证，防止照片/视频攻击</p>
									</div>
									<el-switch v-model="livenessConfig.enabled" :disabled="!livenessConfig.modelStatus.exists" />
								</div>

								<!-- 阈值设置 -->
								<div class="option-row">
									<div class="row-text">
										<p class="label">假体置信度阈值</p>
										<p class="sub">阈值越高，安全性越好，假脸被当作真人的概率越低，建议 0.5~0.7</p>
									</div>
									<el-input-number
										v-model="livenessConfig.threshold"
										:min="0.5"
										:max="0.99"
										:step="0.01"
										:precision="2"
										:disabled="!livenessConfig.enabled"
										style="width: 120px;"
									/>
								</div>

								<div class="option-desc">
									<el-alert title="注意：" type="info" description="阈值越高，对假脸的识别越严格，但真人被误判为假脸的概率也会增加，经测试，该功能对光照要求高，尽量保持较好的光照环境"
						            show-icon :closable="false" />
								</div>
							</section>
						</el-col>
					</el-row>
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

	/* 活体检测相关样式 */
	.model-status-card {
		background: #f5f7fa;
		border-radius: 10px;
		padding: 16px;
		margin-bottom: 20px;
		border: 1px solid #e4e7ed;
	}

	.model-status-card.status-success {
		background: #f0f9eb;
		border-color: #c2e7b0;
	}

	.model-status-card.status-warning {
		background: #fdf6ec;
		border-color: #f5dab1;
	}

	.model-status-card .status-header {
		display: flex;
		align-items: center;
		gap: 8px;
		font-weight: 600;
		margin-bottom: 12px;
		color: #303133;
	}

	.model-status-card .status-content {
		display: flex;
		align-items: center;
		gap: 10px;
		font-size: 14px;
		color: #606266;
	}

	.model-status-card .status-content .status-info {
		display: flex;
		flex-direction: column;
	}

	.model-status-card .status-content .status-info .sub {
		font-size: 12px;
		color: #909399;
		margin-top: 4px;
	}

	.model-status-card .status-success .status-content {
		color: #67c23a;
	}

	.model-status-card .status-warning .status-content {
		color: #e6a23c;
	}

	.option-desc {
		background: #f4f4f5;
		border-radius: 8px;
		padding: 16px;
		margin-top: 20px;
	}

	.option-desc p {
		margin: 6px 0;
		font-size: 13px;
		color: #606266;
	}

	.option-desc code {
		background: #e6a23c;
		color: #fff;
		padding: 2px 6px;
		border-radius: 4px;
		font-size: 12px;
	}

	.info-box {
		background: #f5f7fa;
		border-radius: 10px;
		padding: 20px;
		border: 1px solid #e4e7ed;
	}

	.info-box h5 {
		display: flex;
		align-items: center;
		gap: 8px;
		margin: 16px 0 10px 0;
		font-size: 14px;
		color: #303133;
	}

	.info-box h5:first-child {
		margin-top: 0;
	}

	.info-box p {
		font-size: 13px;
		color: #606266;
		line-height: 1.6;
		margin: 0;
	}

	.info-box ul {
		margin: 10px 0 0 0;
		padding-left: 20px;
	}

	.info-box li {
		font-size: 13px;
		color: #606266;
		line-height: 1.8;
	}

	/* 登录安全设置样式 */
	.password-form {
		background: #f5f7fa;
		border-radius: 10px;
		padding: 20px;
		margin: 16px 0;
		border: 1px solid #e4e7ed;
	}

	.security-tips {
		margin-top: 20px;
	}

	.expire-config {
		background: #f5f7fa;
		border-radius: 10px;
		padding: 20px;
		margin: 16px 0;
		border: 1px solid #e4e7ed;
	}

	.expire-tips {
		margin-top: 16px;
	}

	.session-info {
		display: flex;
		align-items: center;
		gap: 16px;
		padding: 16px;
		background: #f0f9eb;
		border-radius: 10px;
		border: 1px solid #c2e7b0;
		width: 100%;
		box-sizing: border-box;
		flex-wrap: wrap;
	}
</style>