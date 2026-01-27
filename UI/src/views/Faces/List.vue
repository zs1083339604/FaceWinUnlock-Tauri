<script setup lang="ts">
    import { ref, computed } from 'vue';
    import { ElMessageBox, ElMessage } from 'element-plus';
    import { User, Avatar } from '@element-plus/icons-vue';
    import { useRouter } from 'vue-router';
    import { useFacesStore } from '../../stores/faces';
    import { storeToRefs } from 'pinia';

    const router = useRouter();
    const facesStore = useFacesStore();

    const searchQuery = ref('');
    const { faceList } = storeToRefs(facesStore);
    const filteredList = computed(() => {
        return faceList.value.filter(item =>{
            if(item.json_data.alias){
                return item.json_data.alias.includes(searchQuery.value) || item.user_name.includes(searchQuery.value);
            } else {
                return item.user_name.includes(searchQuery.value);
            }
        });
    });

    // 更改view
    const handleView = (face) => {
        facesStore.editFaceJsonData(JSON.stringify({ ...face.json_data, view: !(face.json_data.view)}), face.id).catch(error => {
            ElMessage.warning(error);
        });
    };

    const handleLock = (face) => {
        facesStore.editFaceJsonData(JSON.stringify({ ...face.json_data, lock: !(face.json_data.lock)}), face.id).then(()=>{
            if(face.json_data.lock) {
                ElMessage.success('禁用面容成功');
            }else{
                ElMessage.success('启用面容成功');
            }
        }).catch(error => {
            ElMessage.warning(error);
        });
    };

    // 编辑
    const handleEdit = (face) => {
        router.push({
            path: '/faces/add',
            query: { id: face.id, mode: 'edit' }
        });
    };

    // 删除面容
    const confirmDelete = (face) => {
        ElMessageBox.confirm(`确定要删除面容 [${face.json_data.alias || face.user_name}] 吗？删除后将无法使用该面容解锁系统。`, '警告', {
            confirmButtonText: '确定删除',
            cancelButtonText: '取消',
            type: 'warning',
        }).then(() => {
            facesStore.deleteFace(face.id).then(()=>{
                ElMessage.success('删除成功');
            }).catch((error)=>{
                ElMessage.warning(error);
            })
        });
    };
</script>

<template>
	<div class="face-list-container">
		<div class="list-header">
			<div class="stats">
				<span class="total-text">人脸数量: <strong>{{ faceList.length }}</strong> 个</span>
			</div>
			<div class="actions">
				<el-input v-model="searchQuery" placeholder="搜索备注或用户名..." style="width: 250px; margin-right: 15px"
					prefix-icon="Search" clearable />
				<el-button type="primary" icon="Plus" @click="$router.push('/faces/add')">
					添加新面容
				</el-button>
			</div>
		</div>

		<el-scrollbar v-if="filteredList.length > 0">
			<el-row :gutter="20" style="width: 100%;">
				<el-col v-for="face in filteredList" :key="face.id" :xs="24" :sm="12" :md="8" :lg="6">
					<el-card class="face-card" :class="{ 'disabled': face.json_data.lock }" :body-style="{ padding: '0px' }">
						<div class="face-preview">
                            <div class="disabled-overlay" v-if="face.json_data.lock">
                                <div class="disabled-label">已禁用</div>
                            </div>

                            <div class="face-img-wrapper">
                                <img 
                                    v-face-img="face" 
                                    class="face-img"
                                >
                                <div class="image-slot">
                                    <el-icon><Avatar /></el-icon>
                                </div>
                            </div>
							<div class="card-overlay">
                                <el-button size="small" circle icon="Lock" @click="handleLock(face)" v-if="!face.json_data.lock" title="禁用面容"/>
                                <el-button size="small" circle icon="Unlock" @click="handleLock(face)" v-else title="启用面容"/>
								<el-button size="small" circle icon="View" @click="handleView(face)" v-if="!face.json_data.view" title="显示缩略图"/>
								<el-button size="small" circle icon="Hide" @click="handleView(face)" v-else title="隐藏缩略图" />
								<el-button size="small" circle icon="Edit" @click="handleEdit(face)" title="编辑面容" />
							</div>
						</div>

						<div class="face-info">
							<div class="info-row main">
								<span class="alias">{{ face.json_data.alias ? face.json_data.alias : '无别名' }}</span>
								<el-tag size="small" :type="face.account_type === 'online' ? 'primary' : 'info'">
									{{ face.account_type === 'online' ? '联机' : '本地' }}
								</el-tag>
							</div>
							<div class="info-row sub">
								<el-icon>
									<User />
								</el-icon>
								<span>{{ face.user_name }}</span>
							</div>
							<div class="info-row time">
								<span>注册于: {{ face.createTime }}</span>
							</div>

							<div class="card-footer">
                                <el-button type="danger" variant="light" icon="Delete" size="small" @click="confirmDelete(face)">
                                    删除
                                </el-button>
							</div>
						</div>
					</el-card>
				</el-col>
			</el-row>
		</el-scrollbar>

		<el-empty v-else description="暂无面容数据，请先添加" :image-size="200">
			<el-button type="primary" size="large" @click="$router.push('/faces/add')">
				立即录入面容
			</el-button>
		</el-empty>
	</div>
</template>

<style scoped>
    .list-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        margin-bottom: 25px;
        background: #fff;
        padding: 15px 20px;
        border-radius: 8px;
        box-shadow: 0 2px 12px 0 rgba(0, 0, 0, 0.05);
    }

    .total-text {
        color: #606266;
    }

    .total-text strong {
        color: #409eff;
        font-size: 1.2em;
    }

    .face-card {
        margin-bottom: 20px;
        transition: transform 0.3s;
        border: none;
        overflow: hidden;
        position: relative;
    }

    /* 禁用状态的卡片整体样式 */
    .face-card.disabled {
        opacity: 0.7;
        transform: none;
        box-shadow: none;
        background-color: #faf0f0;
    }

    .face-card:hover {
        transform: translateY(-5px);
        box-shadow: 0 10px 20px rgba(0, 0, 0, 0.1);
    }

    /* 禁用状态hover时取消悬浮效果 */
    .face-card.disabled:hover {
        transform: translateY(0);
        box-shadow: 0 2px 12px 0 rgba(0, 0, 0, 0.1);
    }

    .face-preview {
        height: 160px;
        background: #f5f7fa;
        position: relative;
        display: flex;
        align-items: center;
        justify-content: center;
    }

    /* 禁用状态的预览区样式 */
    .face-card.disabled .face-preview {
        background: #fdf2f2;
    }

    /* 禁用遮罩层 */
    .disabled-overlay {
        position: absolute;
        top: 0;
        left: 0;
        width: 100%;
        height: 160px;
        background: rgba(245, 108, 108, 0.1);
        display: flex;
        align-items: center;
        justify-content: center;
        z-index: 10;
        pointer-events: none; /* 不影响子元素点击 */
    }

    .disabled-label {
        background: #f56c6c;
        color: white;
        padding: 4px 12px;
        border-radius: 4px;
        font-size: 14px;
        font-weight: bold;
        transform: rotate(-15deg);
        box-shadow: 0 2px 8px rgba(245, 108, 108, 0.3);
    }

    .face-img-wrapper {
        width: 100%;
        height: 100%;
        position: relative;
        overflow: hidden;
        display: flex;
        align-items: center;
        justify-content: center;
    }

    .face-img {
        max-width: 100%;
        max-height: 100%;
        object-fit: contain;
        position: absolute;
        top: 0;
        left: 0;
        right: 0;
        bottom: 0;
        margin: auto;
        display: none;
    }

    .face-img[src] {
        display: block;
    }

    .image-slot {
        width: 100%;
        height: 100%;
        display: flex;
        align-items: center;
        justify-content: center;
        color: #dcdfe6;
        font-size: 48px;
        position: absolute;
        top: 0;
        left: 0;
    }

    .face-img[src] + .image-slot {
        display: none;
    }

    .card-overlay {
        position: absolute;
        bottom: 0;
        width: 100%;
        height: 40px;
        background: rgba(0, 0, 0, 0.5);
        display: flex;
        align-items: center;
        justify-content: flex-end;
        padding: 0 10px;
        opacity: 0;
        transition: opacity 0.3s;
        box-sizing: border-box;
        z-index: 20;
    }

    .face-card:hover .card-overlay {
        opacity: 1;
    }

    /* 禁用状态下隐藏操作按钮的hover显示 */
    .face-card.disabled .card-overlay {
        background: rgba(0, 0, 0, 0.3);
    }

    .face-info {
        padding: 15px;
    }

    .face-card.disabled .face-info {
        color: #909399;
    }

    .face-card.disabled .alias {
        color: #909399;
        text-decoration: line-through;
    }

    .info-row {
        display: flex;
        align-items: center;
        gap: 8px;
        margin-bottom: 8px;
        font-size: 13px;
        color: #606266;
    }

    .info-row.main {
        justify-content: space-between;
    }

    .alias {
        font-weight: bold;
        font-size: 15px;
        color: #303133;
    }

    .card-footer {
        margin-top: 15px;
        padding-top: 10px;
        border-top: 1px dashed #ebeef5;
        display: flex;
        justify-content: flex-end;
    }

    .face-card.disabled .card-footer .el-button {
        opacity: 0.8;
    }
</style>