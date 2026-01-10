import { createRouter, createWebHashHistory } from 'vue-router'
import Init from '../views/Init.vue'
import MainLayout from '../layout/MainLayout.vue'
import Dashboard from '../views/Dashboard.vue'
import List from '../views/Faces/List.vue'
import Add from '../views/Faces/Add.vue'
import Options from '../views/Options.vue'
import Logs from '../views/Logs.vue'

const routes = [
	{ path: '/init', name: 'Init', component: Init, meta: { title: '系统初始化' }},
	{ 
		path: '/',
		component: MainLayout,
		children: [
			{
				path: '',
				name: 'Dashboard',
				component: Dashboard,
				meta: { title: '控制仪表盘' }
			},{
				path: 'faces',
				name: 'FaceList',
				component: List,
				meta: { title: '面容库管理' }
			},{
				path: 'faces/add',
				name: 'FaceAdd',
				component: Add,
				meta: { title: '录入/编辑面容' }
			},{
				path: 'options',
				name: 'Options',
				component: Options,
				meta: { title: '首选项' }
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

export default router