import { defineStore } from 'pinia';
import { select, insert, update, deleteData } from '../utils/sqlite';
import { formatObjectString, getCurrentDateTime, removeFace } from '../utils/function'
import { info, error as errorLog, warn } from '@tauri-apps/plugin-log';

export const useFacesStore = defineStore('faces', {
    actions: {
        init(){
            return new Promise((resolve, reject) => {
                select('faces', ['*']).then((result)=>{
                    for(let i = 0; i < result.rows.length; i++){
                        const item = result.rows[i];
                        this.addFaceToList(item);
                    }
                    resolve();
                }).catch((error)=>{
                    errorLog(formatObjectString("面容Store初始化失败：", error));
                    reject(error);
                })
            })
        },
        /**
         * 添加面容
         * @param {Object} data 面容数据
         * @return {Promise} 
         */
        addFace(data){
            return new Promise((resolve, reject) => {
                insert("faces", 
                    ["user_name", "user_pwd", "account_type", "face_token", "json_data"],
                    [data.user_name, data.user_pwd, data.account_type, data.face_token, data.json_data]
                ).then((result)=>{
                    this.addFaceToList({
                        id: result.lastId,
                        createTime: getCurrentDateTime(),
                        ...data
                    });
                    resolve();
                }).catch((error)=>{
                    const info = formatObjectString("添加面容到数据库失败：", error);
                    errorLog(info);
                    reject(error);
                })
            });
        },
        /**
         * 修改面容信息
         * @param {Object} data 面容数据
         * @param {Number} id 面容ID
         * @returns {Promise}
         */
        editFace(data, id){
            return new Promise((resolve, reject) => {
                const faceIndex = this.faceList.findIndex(item => item.id == id);
                if(faceIndex == -1){
                    const info = "未找到id: " + id + " 的面容信息";
                    warn(info);
                    reject(info);
                    return;
                }

                update("faces", 
                    {
                        user_name: data.user_name, user_pwd: data.user_pwd, 
                        account_type: data.account_type, face_token: data.face_token, 
                        json_data: data.json_data
                    },
                    "id = ?",
                    [id]
                ).then(()=>{
                    // 如果解析失败，直接返回失败，后续操作会throw error
                    this.faceList[faceIndex].json_data = JSON.parse(data.json_data);

                    this.faceList[faceIndex].user_name = data.user_name;
                    this.faceList[faceIndex].user_pwd = data.user_pwd;
                    this.faceList[faceIndex].account_type = data.account_type;
                    this.faceList[faceIndex].face_token = data.face_token;
                    resolve();
                }).catch((error)=>{
                    const info = formatObjectString("修改面容到数据库失败：", error);
                    errorLog(info);
                    reject(error);
                })
            })
        },
        /**
         * 修改面容JSON信息
         * @param {String} json_data 面容json信息
         * @param {Number} id 面容ID
         * @returns {Promise}
         */
        editFaceJsonData(json_data, id){
            return new Promise((resolve, reject) => {
                const faceIndex = this.faceList.findIndex(item => item.id == id);
                if(faceIndex == -1){
                    const info = "未找到id: " + id + " 的面容信息";
                    warn(info);
                    reject(info);
                    return;
                }

                update("faces", {json_data: json_data}, "id = ?", [id]).then(()=>{
                    this.faceList[faceIndex].json_data = JSON.parse(json_data);
                    resolve();
                }).catch((error)=>{
                    const info = formatObjectString("修改面容JSON数据到数据库失败：", error);
                    errorLog(info);
                    reject(error);
                })
            })
        },
        /**
         * 传入面容ID 获取面容信息
         * @param {Number} id 面容id
         * @returns {Object} 面容信息
         */
        getFaceById(id){
            let result = null;

            let item = this.faceList.find(item => item.id == id);
            if(item){
                result = {...item};
            }

            return result;
        },
        /**
         * 传入面容ID，返回面容别名
         * @param {Number} id 面容id
         * @returns {String} 面容别名
         */
        getFaceAliasById(id){
            let result = this.getFaceById(id);
            if(result){
                return result.json_data.alias;
            }
            return '无';
        },
        /**
         * 添加面容信息到本地列表
         * @param {Object} data 面容数据（与数据库数据一致）
         */
        addFaceToList(data){
            this.faceList.push({
                id: data.id,
                user_name: data.user_name,
                user_pwd: data.user_pwd,
                account_type: data.account_type,
                face_token: data.face_token,
                json_data: JSON.parse(data.json_data),
                createTime: data.createTime
            });
        },
        /**
         * 删除一条面容数据
         * @param {Number} id 面容ID 
         */
        deleteFace(id){
            return new Promise((resolve, reject) => {
                const faceIndex = this.faceList.findIndex(item => item.id == id);
                if(faceIndex == -1){
                    const info = "未找到id: " + id + " 的面容信息";
                    warn(info);
                    reject(info);
                    return;
                }

                deleteData("faces", "id = ?", [id]).then(()=>{
                    // 面容特征和图片删除失败不影响系统运行
                    removeFace(this.faceList[faceIndex].face_token);
                    this.faceList.splice(faceIndex, 1);
                    resolve();
                }).catch((error)=>{
                    const info = formatObjectString("从数据库删除面容失败：", error);
                    errorLog(info);
                    reject(error);
                })
            })
        }
    },
    state() {
        return{
            faceList: []
        }
    } 
});