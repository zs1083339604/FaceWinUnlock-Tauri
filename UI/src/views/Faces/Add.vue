<script setup lang="ts">
    import { reactive, ref, onMounted, computed, onUnmounted } from 'vue';
    import { ElMessage, ElMessageBox } from 'element-plus';
    import AccountAuthForm from '../../components/AccountAuthForm.vue';
    import { open } from '@tauri-apps/plugin-dialog';
    import { invoke } from '@tauri-apps/api/core';
    import { formatObjectString, removeFace } from '../../utils/function'
    import { openUrl } from '@tauri-apps/plugin-opener';
    import { useRoute, useRouter } from 'vue-router';
    import { info, error as errorLog, warn } from '@tauri-apps/plugin-log';
    import { useFacesStore } from '../../stores/faces';
    import { useOptionsStore } from '../../stores/options';

    const route = useRoute();
    const router = useRouter();
    const facesStore = useFacesStore();
    const optionsStore = useOptionsStore();

    const faceName = ref('');
    const threshold = ref(40);
    // 显示的图片
    const capturedImage = ref('');
    // 这是用来保存的，不要显示
    let rawImageForSystem = '';
    // 是否是摄像头模式
    const isCameraStreaming = ref(false);
    // 是否启用raf循环
    let isLoopRunning = false;
    // 一致性验证模式开关
    const verificationMode = ref(false);
    // 一致性验证模式下的图片
    const verifyingStreamImage = ref('');
    const matchConfidence = ref(0);
    const isProcessing = ref(false);
    // 修改时的面容数据，用于最后提交的判断
    let editFaceData = null;
    // 修改面容时，是否修改了图片
    let isEditFaceImage = false;
    const faceDetectionThreshold = ref(90);

    let authForm = reactive({
        accountType: 'local',
        username: '',
        password: ''
    });

    const isEditMode = computed(() => route.query.mode === 'edit');
    const targetId = route.query.id;

    onMounted(async () => {
        if (isEditMode.value) {
            editFaceData = facesStore.getFaceById(targetId);
            if(editFaceData){
                // 添加账户信息
                authForm.username = editFaceData.user_name;
                authForm.password = editFaceData.user_pwd;
                authForm.accountType = editFaceData.account_type;
                // 添加其他信息
                faceName.value = editFaceData.json_data.alias;
                threshold.value = editFaceData.json_data.threshold;
                faceDetectionThreshold.value = editFaceData.json_data.faceDetectionThreshold * 100;
                // 添加人脸信息
                loadFaceFormPath(localStorage.getItem("exe_dir") + "\\faces\\"+editFaceData.face_token+".faceimg").catch((error)=>{
                    const info = formatObjectString("载入图片失败：", error);
                    errorLog(info);
                    ElMessage.error(info);
                })
            }else{
                ElMessage.warning('未找到该人脸数据');
                router.push('/faces');
            }
        } else {
            // 获取当前用户名
            invoke('get_now_username').then((data)=>{
                if(data.code == 200){
                    authForm.username = data.data.username;
                }
            })
        }
    });

    onUnmounted(async ()=>{
        await stopCamera();
    })

    const handleSelectFile = async () => {
        try {
            const selected = await open({
                multiple: false,
                directory: false,
                filters: [{ name: '图片文件', extensions: ['jpg', 'jpeg', 'png'] }]
            });

            if (!selected) return; 

            isProcessing.value = true;
            
            await loadFaceFormPath(selected);

            isEditFaceImage = true;
        } catch (error) {
            const info = formatObjectString("文件选择失败：", error);
            errorLog(info);
            ElMessage.error(info);
        } finally {
            isProcessing.value = false;
        }
    };

    async function loadFaceFormPath(path){
        const result = await invoke("check_face_from_img", { imgPath: path, faceDetectionThreshold: getFaceDetectionThresholdValue() });
            
        capturedImage.value = result.data.display_base64;
        rawImageForSystem = result.data.raw_base64;

        ElMessage.success('图片载入成功');
    }

    const startCamera = () => {
        let cameraIndex = parseInt(optionsStore.getOptionValueByKey("camera"));
        if(isNaN(cameraIndex)){
            cameraIndex = 0;
        }
        invoke("open_camera", { backend: null, camearIndex: cameraIndex }).then(()=>{
            isCameraStreaming.value = true;
            isLoopRunning = true;
            streamLoop();
        }).catch((error)=>{
            const info = formatObjectString("摄像头开启失败：", error);
            errorLog(info);
            ElMessage.error(formatObjectString(error));
        });
    };

    const streamLoop = async () => {
        if (!isLoopRunning) return;

        try {
            if(!verificationMode.value){
                // 面容录入
                const res = await invoke('check_face_from_camera', {faceDetectionThreshold: getFaceDetectionThresholdValue()});
                capturedImage.value = res.data.display_base64;
                rawImageForSystem = res.data.raw_base64;
            } else {
                // 一致性对比
                const res = await invoke('verify_face', { referenceBase64: rawImageForSystem.split(',')[1], faceDetectionThreshold: getFaceDetectionThresholdValue() });
                if(res.data.display_base64) {
                    verifyingStreamImage.value = res.data.display_base64;
                }

                const rawScore = res.data.score;
                if (rawScore > 0) {
                    matchConfidence.value = Math.floor(Math.min(100, (rawScore / 1.0) * 100));
                } else {
                    matchConfidence.value = 0;
                }
            }

            // 继续下一帧
            requestAnimationFrame(streamLoop);
        } catch (error) {
            const info = formatObjectString("RAF循环出错：" ,error);
            if(info.includes("未检测到人脸")){
                // 这个可以继续，并且不用显示错误
                requestAnimationFrame(streamLoop);
                return;
            }
            errorLog(info);
            ElMessage.error(info);
        }
    };

    const confirmCapture = () => {
        stopCamera().then(()=>{
            if(capturedImage.value && rawImageForSystem){
                isEditFaceImage = true;
            }

            isCameraStreaming.value = false;
        }).catch(()=>{});
    };

    const stopCapture = () => {
        stopCamera().then(()=>{
            isCameraStreaming.value = false;
            capturedImage.value = '';
            rawImageForSystem = '';
        }).catch(()=>{});
    };

    function stopCamera(){
        isLoopRunning = false;
        return new Promise((resolve, reject) => {
            invoke("stop_camera").then(()=>{
                resolve();
            }).catch((error)=>{
                const info = formatObjectString("摄像头关闭失败：", error);
                errorLog(info);
                ElMessage.error(info);
                reject();
            });
        })
    }

    // 切换验证模式
    const toggleVerification = () => {
        verificationMode.value = !verificationMode.value;
        if (verificationMode.value) {
            let cameraIndex = parseInt(optionsStore.getOptionValueByKey("camera"));
            if(isNaN(cameraIndex)){
                cameraIndex = 0;
            }
            invoke("open_camera", { backend: null, camearIndex: cameraIndex }).then(()=>{
                isLoopRunning = true;
                streamLoop();
            }).catch((error)=>{
                const info = formatObjectString("摄像头开启失败：", error);
                errorLog(info);
                ElMessage.error(info);
            });
        } else {
            stopCamera().then(()=>{
                verifyingStreamImage.value = '';
            }).catch(()=>{});
        }
    };

    const handleSave = async () => {
        if (!authForm.username || !authForm.password) {
            ElMessage.warning('请填写完整的账号密码信息')
            return;
        }

        if (!rawImageForSystem) {
            ElMessage.warning('请先录入面容图片');
            return;
        }

        // 判断置信度是否在合理的范围内
        try {
            let messageBoxText = null;
            if(threshold.value < 36){
                messageBoxText = '置信度小于OpenCV推荐的 36%，误判为同一人的可能性很高，是否继续？'
            } else if(threshold.value > 85) {
                messageBoxText = '置信度过高可能会导致误判，是否继续？'
            }

            if(messageBoxText){
                await ElMessageBox.confirm(messageBoxText, '警告',{
                    confirmButtonText: '继续',
                    cancelButtonText: '取消',
                    type: 'warning',
                });
            }
            
        } catch (error) {
            return;
        }
    
        if(isEditMode.value){
            // 如果是修改，判断数据是否完全一致
            if(
                authForm.username == editFaceData.user_name &&
                authForm.password == editFaceData.user_pwd &&
                authForm.accountType == editFaceData.account_type &&
                faceName.value == editFaceData.json_data.alias &&
                threshold.value == editFaceData.json_data.threshold &&
                getFaceDetectionThresholdValue() == editFaceData.json_data.faceDetectionThreshold &&
                !isEditFaceImage
            ){
                // 没有任何变化，直接成功
                ElMessage.success('修改成功！');
                router.push('/faces');
                return;
            }
        }

        isProcessing.value = true;

        let face_token = "";

        if(isEditMode.value && !isEditFaceImage){
            // 如果编辑模式中，没有修改图片，则不用重新存储面容特征
            face_token = editFaceData.face_token;
        }else{
            // 如果非编辑模式，或者编辑模式修改了图片
            try {
                const result = await invoke("save_face_registration", {name: faceName.value || '', referenceBase64: rawImageForSystem.split(',')[1], faceDetectionThreshold: getFaceDetectionThresholdValue()});
                face_token = result.data.file_name;
            } catch (error) {
                const info = formatObjectString("存储面容失败：", error);
                errorLog(info);
                ElMessage.error(info);
                isProcessing.value = false;
                return;
            }
        }

        try {

            if(!isEditMode.value){
                await facesStore.addFace({
                    "user_name": authForm.username,
                    "user_pwd": authForm.password,
                    "account_type": authForm.accountType,
                    "face_token": face_token,
                    "json_data": JSON.stringify({
                        threshold: threshold.value,
                        alias: faceName.value || '',
                        view: true,
                        faceDetectionThreshold: getFaceDetectionThresholdValue()
                    })
                });
            } else {
                await facesStore.editFace({
                    "user_name": authForm.username,
                    "user_pwd": authForm.password,
                    "account_type": authForm.accountType,
                    "face_token": face_token,
                    "json_data": JSON.stringify({
                        threshold: threshold.value,
                        alias: faceName.value || '',
                        view: editFaceData.json_data.view ? editFaceData.json_data.view : true,
                        faceDetectionThreshold: getFaceDetectionThresholdValue()
                    })
                }, targetId);

                if(isEditFaceImage){
                    // 如果信息存储完成，并且修改了图片，删除旧的面容特征
                    // 删除不成功，也不影响使用，所以不用退出
                    removeFace(editFaceData.face_token, "删除旧面容");
                }
            }
            
            info(`${authForm.username} 面容${isEditMode.value ? '修改' : '添加'}成功！`);
            ElMessage.success(isEditMode.value ? '修改成功' : '添加成功');
            router.push('/faces');
        } catch (error) {
            // 如果失败 删除上面生成的面容图片和特征文件
            removeFace(face_token);
            ElMessage.error(error);
        } finally {
            isProcessing.value = false;
        }
    };

    // 处理 faceDetectionThreshold 的值，确保 / 100 在2位小数之间
    // JS的除法真的不敢恭维，太不靠谱了
    function getFaceDetectionThresholdValue(){
        return parseFloat((faceDetectionThreshold.value / 100).toFixed(2));
    }
</script>

<template>
    <div class="face-add-container">
        <el-row :gutter="24">
            <el-col :span="14">
                <el-card class="visual-card" shadow="never">
                    <div class="display-container" :class="{ 'split-view': verificationMode }">

                        <div class="screen-box primary-screen">
                            <div class="screen-label">{{ verificationMode ? '参考底库' : '采集预览' }}</div>
                            <div v-if="!capturedImage" class="placeholder-content">
                                <el-icon :size="48">
                                    <UserFilled />
                                </el-icon>
                                <p>待录入面容</p>
                            </div>
                            <img v-else :src="capturedImage" class="result-img" />
                        </div>

                        <div v-if="verificationMode" class="screen-box secondary-screen">
                            <div class="screen-label">实时验证流</div>
                            <div class="scanner-line"></div>
                            <div v-if="!verifyingStreamImage" class="camera-stream-mock">
                                <el-icon :size="48" class="is-loading">
                                    <Loading />
                                </el-icon>
                            </div>
                            <img v-else :src="verifyingStreamImage" class="result-img" />
                            <div class="confidence-tag" :class="matchConfidence > (threshold) ? 'match' : 'mismatch'">
                                相似度: {{ matchConfidence }}%
                            </div>
                        </div>
                    </div>

                    <div class="action-bar">
                        <div class="detection-config">
                            <span class="label">检测灵敏度</span>
                            <el-slider 
                                v-model="faceDetectionThreshold" 
                                :min="10" 
                                :max="100" 
                                size="small"
                            />
                            <el-tooltip content="控制摄像头识别出人脸的难易程度" placement="top">
                                <el-icon :size="14" style="margin-left: 5px; cursor: help;"><QuestionFilled /></el-icon>
                            </el-tooltip>
                        </div>
                        <div class="capture-controls" v-if="!verificationMode">
                            <template v-if="!isCameraStreaming">
                                <el-button 
                                    type="primary" 
                                    plain 
                                    icon="Picture" 
                                    @click="handleSelectFile"
                                    :loading="isProcessing"
                                >
                                    选择本地照片
                                </el-button>
                                <el-button type="primary" @click="startCamera" :loading="isProcessing">从摄像头抓拍</el-button>
                            </template>
                            <template v-else>
                                <el-button type="success" icon="Check" @click="confirmCapture">确认抓拍</el-button>
                                <el-button type="danger" plain icon="Close" @click="stopCapture">取消</el-button>
                            </template>
                        </div>

                        <div class="verify-controls" v-else>
                            <el-tag type="info" effect="plain">正在进行一致性验证...</el-tag>
                        </div>

                        <el-button v-if="capturedImage && !isCameraStreaming" :type="verificationMode ? 'danger' : 'warning'"
                            @click="toggleVerification">
                            {{ verificationMode ? '停止验证' : '一致性验证' }}
                        </el-button>
                    </div>
                </el-card>
            </el-col>

            <el-col :span="10">
                <el-card shadow="never">
                    <template #header><span class="font-bold">底库配置</span></template>
                    <el-form label-position="top">
                        <el-form-item label="面容别名">
                            <el-input v-model="faceName" placeholder="如：XX设备录入" />
                        </el-form-item>

                        <el-form-item label="判定阈值 (置信度)">
                            <div class="slider-box">
                                <el-slider v-model="threshold" :min="20" :max="100" style="width: 100%;"/>
                                <el-tooltip
                                    content="<span>OpenCV 官网建议 <strong>≥ 0.363</strong> (约 <strong>36%</strong>) <br />单击以打开 OpenCV 文档</span>"
                                    placement="top-end"
                                    raw-content
                                >
                                    <el-icon class="question-icon" @click="openUrl('https://docs.opencv.org/4.x/d0/dd4/tutorial_dnn_face.html')"><QuestionFilled /></el-icon>
                                </el-tooltip>
                            </div>
                            <!-- 26-01-04 感觉tip占用空间点有大，尽量让内容在一屏中 -->
                            <!-- <div class="tip">
                                当前阈值: <b style="color: #606266; margin: 0 4px;">{{ threshold }}%</b>
                                <span @click="openUrl('https://docs.opencv.org/4.x/d0/dd4/tutorial_dnn_face.html')">
                                    OpenCV 官网建议 ≥ 0.363 (约 36%)
                                </span>
                            </div> -->
                        </el-form-item>

                        <el-divider>关联系统账户</el-divider>
                        <AccountAuthForm v-model="authForm" :small="true" :customTips="'此密码仅用于 DLL 调起 WinLogon 认证<br />不会上传至任何云端<br />注意：<strong>当前使用明文存储</strong>'"/>

                        <div class="footer-btns">
                            <el-button type="success" size="large" @click="handleSave" :disabled="!capturedImage || isCameraStreaming" :loading="isProcessing">
                                {{ isEditMode ? '确认修改' : '保存并录入系统' }}
                            </el-button>
                        </div>
                    </el-form>
                </el-card>
            </el-col>
        </el-row>
    </div>
</template>

<style scoped>
    .display-container {
        display: flex;
        gap: 10px;
        height: 320px;
        background: #000;
        border-radius: 8px;
        overflow: hidden;
        transition: all 0.3s ease;
    }

    .screen-box {
        flex: 1;
        position: relative;
        display: flex;
        justify-content: center;
        align-items: center;
        background: #1a1a1a;
        border: 1px solid #333;
    }

    .screen-label {
        position: absolute;
        top: 10px;
        left: 10px;
        background: rgba(0, 0, 0, 0.6);
        color: #fff;
        padding: 2px 8px;
        font-size: 12px;
        border-radius: 4px;
        z-index: 5;
    }

    .result-img {
        max-width: 100%;
        max-height: 100%;
        object-fit: contain;
        filter: drop-shadow(0 0 8px rgba(0, 242, 255, 0.2));
        border: 1px solid #333;
    }

    .placeholder-content {
        color: #444;
        text-align: center;
    }

    /* 验证模式下的分割线效果 */
    .split-view .screen-box {
        flex: 0 0 calc(50% - 5px);
    }

    .camera-stream-mock {
        width: 100%;
        height: 100%;
        display: flex;
        justify-content: center;
        align-items: center;
        color: #409eff;
    }

    .scanner-line {
        position: absolute;
        width: 100%;
        height: 2px;
        background: rgba(64, 158, 255, 0.5);
        box-shadow: 0 0 10px #409eff;
        animation: scan 2s infinite ease-in-out;
        z-index: 2;
    }

    .confidence-tag {
        position: absolute;
        bottom: 20px;
        padding: 5px 15px;
        border-radius: 20px;
        font-weight: bold;
        font-size: 14px;
    }

    .match {
        background: #67c23a;
        color: white;
    }

    .mismatch {
        background: #f56c6c;
        color: white;
    }

    .detection-config {
        display: flex;
        align-items: center;
        background: #f0f2f5;
        padding: 5px 12px;
        border-radius: 4px;
        gap: 10px;
        width: 100%;
    }

    .detection-config .label {
        font-size: 12px;
        color: #606266;
        white-space: nowrap;
    }

    .action-bar {
        margin-top: 20px;
        display: flex;
        justify-content: space-between;
        align-items: center;
        flex-wrap: wrap;
        gap: 10px;
    }

    .footer-btns {
        margin-top: 20px;
    }

    .tip {
        font-size: 13px;
        color: #909399;
        margin-top: 8px;
        display: flex;
        align-items: center;
    }

    .tip span {
        margin-left: 8px;
        color: #409eff;
        cursor: pointer;
        text-decoration: underline;
        transition: color 0.2s ease;
        text-underline-offset: 3px;
    }

    .tip span:hover {
        color: #66b1ff;
        text-decoration: none;
    }

    .slider-box{
        width: 100%;
        display: flex;
        align-items: center;
    }

    .question-icon{
        margin-left: 10px;
        font-size: 16px;
        cursor: pointer;
    }

    @keyframes scan {
        0% {
            top: 10%;
        }

        50% {
            top: 90%;
        }

        100% {
            top: 10%;
        }
    }
</style>