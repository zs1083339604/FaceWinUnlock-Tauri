import { selectCustom } from '../utils/sqlite';
import { info, error as errorLog, warn } from '@tauri-apps/plugin-log';
import { formatObjectString } from '../utils/function';

export function useUnlockLog() {
    /**
     * 查询全部解锁日志
     * @returns {Promise<Array>} 全部日志数据
     */
    const queryAllLogs = () => {
        return new Promise((resolve, reject) => {
            selectCustom(
                'SELECT * FROM unlock_log ORDER BY lastTime DESC',
                []
            ).then((result) => {
                resolve(result.rows || []);
            }).catch((error) => {
                const info = formatObjectString("查询全部解锁日志失败：", error);
                errorLog(info);
                reject(error);
            })
        })
    };

    /**
     * 分页查询解锁日志
     * @param {number} page - 页码
     * @param {number} pageSize - 每页条数
     * @returns {Promise<Array>} 分页后的日志数据
     */
    const queryLogsByPage = (page, pageSize) => {
        return new Promise((resolve, reject) => {
            // 校验参数合法性
            if (page < 1 || pageSize < 1) {
                const info = '页码和每页条数必须大于0';
                errorLog(info);
                reject(error);
                return;
            }

            const offset = (page - 1) * pageSize;
            let total = 0;
            // 先查询总数
            selectCustom("SELECT COUNT(id) AS total FROM unlock_log;").then((result)=>{
                total = result.rows[0].total;
                return selectCustom(
                    'SELECT * FROM unlock_log ORDER BY lastTime DESC LIMIT ? OFFSET ?',
                    [pageSize, offset]
                );
            }).then((result) => {
                resolve({
                    total,
                    list: result.rows || []
                });
            }).catch((error)=>{
                const info = formatObjectString(`分页查询解锁日志失败（第${page}页，每页${pageSize}条）：`, error);
                errorLog(info);
                reject(error);
            })
        });
    };

    /**
     * 查询今日的解锁日志
     * @returns {Promise<Array>} 今日的日志数据
     */
    const queryTodayLogs = () => {
        return new Promise((resolve, reject) => {
            selectCustom(
                `SELECT * FROM unlock_log 
                WHERE date(lastTime) = date('now', 'localtime') 
                ORDER BY lastTime DESC`,
                []
            ).then((result) => {
                resolve(result.rows || []);
            }).catch((error) => {
                const info = formatObjectString("查询今日解锁日志失败：", error);
                errorLog(info);
                reject(error);
            });
        });
    };

    /**
     * 查询指定日期的解锁日志
     * @param {string} date - 要查询的日期（格式：YYYY-MM-DD）
     * @returns {Promise<Array>} 指定日期的日志数据
     */
    const queryLogsByDate = (date) => {
        return new Promise((resolve, reject) => {
            // 验证日期格式
            const dateRegex = /^\d{4}-\d{2}-\d{2}$/;
            if (!dateRegex.test(date)) {
                const info = '日期格式错误，请使用 YYYY-MM-DD 格式';
                errorLog(info);
                reject(error);
                return;
            }

            selectCustom(
                `SELECT * FROM unlock_log 
                WHERE date(lastTime) = ? 
                ORDER BY lastTime DESC`,
                [date]
            ).then((result) => {
                resolve(result.rows || []);
            }).catch((error) => {
                const info = formatObjectString(`查询${date}解锁日志失败：`, error);
                errorLog(info);
                reject(error);
            });
        });
    };

    return {
        queryAllLogs,
        queryLogsByPage,
        queryTodayLogs,
        queryLogsByDate
    };
}