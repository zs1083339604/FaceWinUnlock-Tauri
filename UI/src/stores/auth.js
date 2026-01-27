import { defineStore } from 'pinia';
import { invoke } from '@tauri-apps/api/core';
import { info, warn } from '@tauri-apps/plugin-log';
import { formatObjectString } from '../utils/function';

export const useAuthStore = defineStore('auth', {
    state() {
        return {
            isLoggedIn: false,
            loginEnabled: false,
            passwordHash: '',
            loginTime: null,
            expireMinutes: 0 // 登录过期时间（分钟），0表示退出应用才需要重新登录
        }
    },

    getters: {
        isLoginEnabled: (state) => state.loginEnabled && state.passwordHash,
        // 获取过期时间（毫秒），0表示永不过期（退出应用才需要重新登录）
        expireTimeMs: (state) => {
            if (state.expireMinutes <= 0) return 0;
            return state.expireMinutes * 60 * 1000;
        }
    },

    actions: {
        /**
         * 获取过期时间选项（分钟）
         */
        getExpireOptions() {
            return [
                { label: '无超时（仅打开应用时验证）', value: 0 },
                { label: '15分钟', value: 15 },
                { label: '30分钟', value: 30 },
                { label: '1小时', value: 60 },
                { label: '2小时', value: 120 }
            ];
        },

        /**
         * 设置登录过期时间
         * @param {Number} minutes 过期时间（分钟），0表示退出应用才需要重新登录
         */
        setExpireMinutes(minutes) {
            this.expireMinutes = minutes;
            localStorage.setItem('auth_expire_minutes', minutes.toString());
        },

        /**
         * 获取登录过期时间
         */
        getExpireMinutes() {
            const stored = localStorage.getItem('auth_expire_minutes');
            return stored ? parseInt(stored, 10) : 0;
        },

        /**
         * 初始化认证状态
         */
        async init() {
            try {
                // 获取登录设置
                const loginEnabled = localStorage.getItem('auth_login_enabled') === 'true';
                const passwordHash = localStorage.getItem('auth_password_hash') || '';
                const expireMinutes = this.getExpireMinutes();

                this.loginEnabled = loginEnabled;
                this.passwordHash = passwordHash;
                this.expireMinutes = expireMinutes;

                // 如果启用了登录，每次打开应用都需要重新登录
                if (loginEnabled && passwordHash) {
                    // 每次打开应用都需要重新登录，不记住登录状态
                    this.isLoggedIn = false;
                    this.loginTime = null;
                    localStorage.removeItem('auth_login_time');
                    info('登录已启用，每次打开应用需要重新验证密码');
                    return false;
                }

                // 未启用登录
                this.isLoggedIn = true;
                info('登录未启用，直接进入应用');
                return true;
            } catch (error) {
                warn(formatObjectString('认证状态初始化失败', error));
                return true; // 出错时允许进入，避免无法使用
            }
        },

        /**
         * 检查登录是否过期
         * @returns {Boolean} true 表示已过期，需要重新登录
         */
        isExpired() {
            // 如果没有登录时间，或者过期时间为0（退出应用才需要重新登录），则不标记为过期
            if (!this.loginTime || this.expireMinutes <= 0) {
                return false;
            }
            // 检查是否超过过期时间
            const elapsed = Date.now() - this.loginTime;
            return elapsed > this.expireTimeMs;
        },

        /**
         * 执行登录
         * @param {String} password 用户输入的密码
         */
        async login(password) {
            try {
                // 验证密码
                const result = await invoke('verify_app_password', { password });
                if (result.code === 200) {
                    this.isLoggedIn = true;
                    this.loginTime = Date.now();
                    localStorage.setItem('auth_login_time', this.loginTime.toString());
                    info('登录成功');
                    return true;
                } else {
                    warn('密码验证失败');
                    return false;
                }
            } catch (error) {
                warn(formatObjectString('登录验证失败', error));
                return false;
            }
        },

        /**
         * 设置登录密码
         * @param {String} password 新密码
         */
        async setPassword(password) {
            try {
                const result = await invoke('hash_password', { password });
                if (result.code === 200) {
                    this.passwordHash = result.data.hash;
                    this.loginEnabled = true;
                    localStorage.setItem('auth_password_hash', this.passwordHash);
                    localStorage.setItem('auth_login_enabled', 'true');
                    info('登录密码设置成功');
                    return true;
                } else {
                    warn('密码设置失败');
                    return false;
                }
            } catch (error) {
                warn(formatObjectString('设置密码失败', error));
                return false;
            }
        },

        /**
         * 清除登录密码
         */
        clearPassword() {
            this.passwordHash = '';
            this.loginEnabled = false;
            this.isLoggedIn = true; // 清除后仍可访问
            localStorage.removeItem('auth_password_hash');
            localStorage.removeItem('auth_login_enabled');
            info('登录密码已清除');
        },

        /**
         * 登出
         */
        logout() {
            this.isLoggedIn = false;
            this.loginTime = null;
            localStorage.removeItem('auth_login_time');
            info('用户已登出');
        },

        /**
         * 清除登录状态（移除登录状态）
         */
        clearLoginState() {
            this.isLoggedIn = false;
            this.loginTime = null;
            localStorage.removeItem('auth_login_time');
            info('登录状态已移除');
        },

        /**
         * 检查是否需要登录
         */
        shouldRequireLogin() {
            return this.loginEnabled && this.passwordHash && !this.isLoggedIn;
        }
    }
});
