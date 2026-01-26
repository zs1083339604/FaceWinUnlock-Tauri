import { createRouter, createWebHashHistory } from 'vue-router'
import Init from '../views/Init.vue'
import Login from '../views/Login.vue'
import MainLayout from '../layout/MainLayout.vue'
import Dashboard from '../views/Dashboard.vue'
import List from '../views/Faces/List.vue'
import Add from '../views/Faces/Add.vue'
import Options from '../views/Options.vue'
import Logs from '../views/Logs.vue'
import { useAuthStore } from '../stores/auth'

const routes = [
	{ path: '/init', name: 'Init', component: Init, meta: { title: '系统初始化' }},
	{ path: '/login', name: 'Login', component: Login, meta: { title: '登录', public: true }},
	{ 
		path: '/',
		component: MainLayout,
		meta: { requiresAuth: true },
		children: [
			{
				path: '',
				name: 'Dashboard',
				component: Dashboard,
				meta: { title: '概览' }
			},{
				path: 'faces',
				name: 'FaceList',
				component: List,
				meta: { title: '面容管理' }
			},{
				path: 'faces/add',
				name: 'FaceAdd',
				component: Add,
				meta: { title: '录入/编辑面容' }
			},{
				path: 'options',
				name: 'Options',
				component: Options,
				meta: { title: '设置' }
			},{
				path: 'logs',
				name: 'Logs',
				component: Logs,
				meta: { title: '日志' }
			}
		]
	}
]

const router = createRouter({
	history: createWebHashHistory(),
	routes
});

// 路由守卫
router.beforeEach(async (to, from, next) => {
	const authStore = useAuthStore();
	
	// 如果是公开路由，直接放行
	if (to.meta.public) {
		next();
		return;
	}

	// 检查是否需要认证
	if (to.meta.requiresAuth) {
		// 初始化认证状态
		if (!authStore.loginEnabled && !authStore.passwordHash) {
			// 未启用登录，直接放行
			next();
			return;
		}

		// 检查是否已登录
		if (authStore.isLoggedIn) {
			// 检查登录是否过期
			if (authStore.isExpired()) {
				// 登录已过期，清除登录状态并跳转到登录页
				authStore.clearLoginState();
				next({
					name: 'Login',
					query: { redirect: to.fullPath, expired: 'true' }
				});
				return;
			}
			next();
			return;
		}

		// 需要登录，跳转到登录页
		next({
			name: 'Login',
			query: { redirect: to.fullPath }
		});
		return;
	}

	next();
});

export default router
