<script setup>
	import { ref } from 'vue';
	import { RouterView } from 'vue-router';
	import { connect } from './utils/sqlite.js';
	import { formatObjectString } from './utils/function.js';
	import { getCurrentWindow } from '@tauri-apps/api/window';
	import { ElMessageBox } from 'element-plus';
	import { useOptionsStore } from "./stores/options";
	import { useRouter } from 'vue-router';
	import { invoke } from '@tauri-apps/api/core';
	import { info, warn } from '@tauri-apps/plugin-log';
	import { useFacesStore } from './stores/faces.js';
	import { attachConsole } from "@tauri-apps/plugin-log";
	import { resourceDir } from '@tauri-apps/api/path';

	const isInit = ref(false);
	const router = useRouter();
	const optionsStore = useOptionsStore();
	const facesStore = useFacesStore();
	const currentWindow = getCurrentWindow();

	// 打包时注释
	attachConsole();

	resourceDir().then((result)=>{
		localStorage.setItem('exe_dir', result);
		return connect();
	}).then(()=>{
		return optionsStore.init();
	}).then(()=>{
		return invoke("init_model");
	}).then(()=>{
		return facesStore.init();
	}).then(()=>{
		let is_initialized = optionsStore.getOptionByKey('is_initialized');
		if(is_initialized.index == -1 || is_initialized.data.val != 'true'){
			warn("程序未初始化，强制跳转初始化界面");
			router.push('/init');
		}
		info("程序初始化完成");
		isInit.value = true;
	}).catch((error)=>{
		ElMessageBox.alert(formatObjectString(error), '程序初始化失败', {
			confirmButtonText: '确定',
			callback: (action) => {
				currentWindow.close();
			}
		});
	})
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