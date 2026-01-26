<script setup>
import { ref, reactive, computed, nextTick } from 'vue';
import { useRouter } from 'vue-router';
import { ElMessage } from 'element-plus';
import { invoke } from '@tauri-apps/api/core';
import { useAuthStore } from '../stores/auth';

const router = useRouter();
const authStore = useAuthStore();

const version = ref(localStorage.getItem("version") || 'unknown');

const loginForm = reactive({
    password: ''
});

const isLoading = ref(false);
const errorMessage = ref('');

// 是否启用了应用登录
const isLoginEnabled = computed(() => authStore.isLoginEnabled);

const handleLogin = async () => {
    if (!loginForm.password) {
        errorMessage.value = '请输入密码';
        return;
    }

    errorMessage.value = '';
    isLoading.value = true;

    try {
        const result = await invoke('verify_app_password', { password: loginForm.password });
        if (result.code === 200) {
            // 先更新登录状态，确保路由守卫能正确检查
            await authStore.login(loginForm.password);
            ElMessage.success('登录成功');
            // 使用 nextTick 确保状态更新后再跳转
            await nextTick();
            router.replace('/');
        } else {
            errorMessage.value = result.msg || '密码错误';
        }
    } catch (error) {
        errorMessage.value = typeof error === 'string' ? error : '登录失败，请重试';
    } finally {
        isLoading.value = false;
    }
};

// 回车登录
const handleKeyDown = (e) => {
    if (e.key === 'Enter') {
        handleLogin();
    }
};
</script>

<template>
    <div class="login-container">
        <div class="login-box">
            <div class="login-header">
                <div class="logo">
                    <svg viewBox="0 0 24 24" width="48" height="48" fill="currentColor">
                        <path d="M12 1L3 5v6c0 5.55 3.84 10.74 9 12 5.16-1.26 9-6.45 9-12V5l-9-4zm0 10.99h7c-.53 4.12-3.28 7.79-7 8.94V12H5V6.3l7-3.11v8.8z"/>
                    </svg>
                </div>
                <h1>FaceWinUnlock</h1>
                <p class="subtitle">面容解锁助手</p>
            </div>

            <div class="login-form">
                <div class="welcome-text">
                    {{ isLoginEnabled ? '请输入应用登录密码' : '欢迎使用 FaceWinUnlock' }}
                </div>

                <el-input
                    v-model="loginForm.password"
                    type="password"
                    placeholder="请输入密码"
                    show-password
                    @keydown="handleKeyDown"
                    :disabled="isLoading"
                    size="large"
                />

                <div v-if="errorMessage" class="error-message">
                    {{ errorMessage }}
                </div>

                <el-button
                    type="primary"
                    size="large"
                    :loading="isLoading"
                    @click="handleLogin"
                    class="login-btn"
                >
                    登录
                </el-button>

            </div>

            <div class="login-footer">
                <span>v {{ version }}</span>
            </div>
        </div>
    </div>
</template>

<style scoped>
.login-container {
    height: 100vh;
    width: 100vw;
    display: flex;
    justify-content: center;
    align-items: center;
    background: #f0f2f7;
}

.login-box {
    width: 100%;
    max-width: 420px;
    background: white;
    border-radius: 12px;
    padding: 36px;
    box-shadow: 0 8px 16px rgba(0, 0, 0, 0.1);
}

.login-header {
    text-align: center;
    margin-bottom: 24px;
}

.logo {
    color: #6c63ff;
    margin-bottom: 12px;
}

.login-header h1 {
    font-size: 24px;
    font-weight: 600;
    color: #222;
    margin: 0 0 8px 0;
}

.subtitle {
    color: #888;
    font-size: 14px;
    margin: 0;
}

.welcome-text {
    font-size: 16px;
    color: #333;
    margin-bottom: 16px;
    text-align: center;
}

.error-message {
    color: #f56c6c;
    font-size: 14px; 
    margin: 8px 0;
    text-align: center;
}

.login-btn {
    width: 100%;
    margin-top: 16px; 
    height: 44px;
    font-size: 16px;
    border-radius: 8px; 
}

.tips {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    margin-top: 16px; 
    padding: 12px;
    background: #e6f7ff; 
    border-radius: 8px;
    font-size: 13px; 
    color: #1890ff; 
    line-height: 1.6;
}

.login-footer {
    text-align: center;
    margin-top: 20px; 
    padding-top: 16px;
    border-top: 1px solid #eee;
    color: #777;
    font-size: 12px;
}
</style>
