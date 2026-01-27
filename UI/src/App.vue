<script setup>
	import { ref, onMounted } from 'vue';
	import { RouterView, useRouter } from 'vue-router';
	import { connect } from './utils/sqlite.js';
	import { formatObjectString } from './utils/function.js';
	import { ElMessageBox } from 'element-plus';
	import { useOptionsStore } from "./stores/options";
	import { info, warn } from '@tauri-apps/plugin-log';
	import { useFacesStore } from './stores/faces.js';
	import { useAuthStore } from './stores/auth.js';
	import { attachConsole } from "@tauri-apps/plugin-log";
	import { resourceDir } from '@tauri-apps/api/path';
	import { getVersion } from '@tauri-apps/api/app';
	import { getCurrentWindow } from '@tauri-apps/api/window';
	import { invoke } from '@tauri-apps/api/core';

	const isInit = ref(false);
	const router = useRouter();
	const optionsStore = useOptionsStore();
	const facesStore = useFacesStore();
	const authStore = useAuthStore();
	const currentWindow = getCurrentWindow();

	// 打包时注释
	attachConsole();
	// 初始化认证状态
	const initAuth = async () => {
		await authStore.init();
	};

	resourceDir().then((result)=>{
		localStorage.setItem('exe_dir', result);
		return connect();
	}).then(async () => {
		// 初始化认证
		await initAuth();
		
		return optionsStore.init();
	}).then(()=>{
		return invoke("init_model");
	}).then(()=>{
		return facesStore.init();
	}).then(async () => {
		// 初始化认证
		await initAuth();
		
		let is_initialized = optionsStore.getOptionByKey('is_initialized');
		if(is_initialized.index == -1 || is_initialized.data.val != 'true'){
			warn("程序未初始化，强制跳转初始化界面");
			router.push('/init');
			isInit.value = true;
			return;
		}

		// 检查是否需要登录
		if (authStore.shouldRequireLogin()) {
			info("需要登录，强制跳转登录界面");
			router.push('/login');
		} else {
			info("程序初始化完成");
		}

		if(optionsStore.getOptionValueByKey('silentRun') != "true"){
			currentWindow.isVisible().then((visible) => {
				if(!visible){
					currentWindow.show();
				}
				currentWindow.setFocus();
			}).catch((error)=>{
				warn(formatObjectString("获取窗口状态失败 ",error));
			})
		}
		isInit.value = true;
	}).catch((error)=>{
		ElMessageBox.alert(formatObjectString(error), '程序初始化失败', {
			confirmButtonText: '确定',
			callback: (action) => {
				invoke("close_app");
			}
		});
	})


	// 版本号不影响运行，不用放在上面
	getVersion().then((v)=>{
		localStorage.setItem('version', v);
	});
</script>

<template>
	<div class="app-wrapper" v-if="isInit">
		<router-view />
    </div>
</template>

<style scoped>
	.app-wrapper {
		height: 100vh;
		width: 100vw;
	}
</style>
