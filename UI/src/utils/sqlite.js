import Database from '@tauri-apps/plugin-sql';
import { formatObjectString } from './function'
import { info, error as errorLog, warn } from '@tauri-apps/plugin-log';
import { executableDir, join } from '@tauri-apps/api/path';

let db = null;
let isConnect = false;

// 基础的数据表
const databseTable = [
    {
        // 设置
        name: 'options',
        columns: [
            { name: 'id', type: 'INTEGER', primaryKey: true, autoIncrement: true, notNull: true },
            // 设置的key
            { name: 'key', type: 'TEXT', notNull: true, unique: true },
            // 设置key所对应的值
            { name: 'val', type: 'TEXT', notNull: true },
            // 上次更新时间
            { name: 'lastTime', type: 'TEXT', defaultValue: "datetime('now', 'localtime')" }
        ]
    },{
        // 面容表
        name: 'faces',
        columns: [
            { name: 'id', type: 'INTEGER', primaryKey: true, autoIncrement: true, notNull: true },
            // 对应的Windows用户名
            { name: 'user_name', type: 'TEXT', notNull: true },
            // 加密后的Windows密码（由DLL用来调起登录）
            { name: 'user_pwd', type: 'TEXT', notNull: true },
            // windows账户类型
            { name: 'account_type', type: 'TEXT', notNull: true },
            // 人脸特征标识符和图片的存储路径
            // 特征是 .face 图片是 .faceimg
            { name: 'face_token', type: 'TEXT', notNull: true },
            // 其余JSON数据，便于随时添加
            // alias 面容别名
            // threshold 置信度
            // view 是否在列表页显示图片缩略图
            // faceDetectionThreshold 人脸的置信度
            { name: 'json_data', type: 'TEXT', notNull: true },
            // 创建时间
            { name: 'createTime', type: 'TEXT', defaultValue: "datetime('now', 'localtime')" }
        ]
    },{
        // 解锁记录
        name: 'unlock_log',
        columns: [
            { name: 'id', type: 'INTEGER', primaryKey: true, autoIncrement: true, notNull: true },
            // 面容ID
            { name: 'face_id', type: 'INTEGER' },
            // 是否成功解锁
            { name: 'is_unlock', type: 'INTEGER', notNull: true },
            // 上次更新时间
            { name: 'lastTime', type: 'TEXT', defaultValue: "datetime('now', 'localtime')" }
        ]
    }
];

/**
 * 根据结构化定义生成 CREATE TABLE 语句
 * @param {object} tableDef - 表定义对象
 * @returns {string} CREATE TABLE 语句
 */
function generateCreateSql(tableDef) {
    const columnsSql = tableDef.columns.map(col => {
        let sql = `${col.name} ${col.type}`;
        if (col.notNull) sql += ' NOT NULL';
        if (col.primaryKey) sql += ' PRIMARY KEY';
        if (col.autoIncrement) sql += ' AUTOINCREMENT';
        if (col.unique) sql += ' UNIQUE';
        if (col.defaultValue !== undefined) sql += ` DEFAULT (${col.defaultValue})`;
        return sql;
    }).join(',\n\t\t\t');
    
    return `CREATE TABLE ${tableDef.name}(\n\t\t\t${columnsSql}\n\t\t);`;
}

/**
 * 建立数据库连接并初始化所有表结构
 */
async function connect() {
    if (isConnect) {
        return; // 已连接则直接返回一个已解析的Promise
    }
    try {

        // 建立连接
        db = await Database.load(`sqlite:${localStorage.getItem("exe_dir")}\\database.db`);
        
        // 遍历所有表定义并进行初始化/同步
        for (const tableDef of databseTable) {
            await initTableSchema(tableDef);
        }
        
        // 标记连接成功
        isConnect = true;
    } catch (error) {
        // 不管在何时发生了错误，直接退出
        db = null;
        isConnect = false;
        errorLog(formatObjectString("数据库连接或初始化失败: ", error));
        throw error;
    }
}

/**
 * 检查并同步单个表的 Schema
 * @param {object} tableDef - 期望的表结构定义
 */
async function initTableSchema(tableDef) {
    const tableName = tableDef.name;
    const expectedColumns = tableDef.columns.map(c => ({ 
        name: c.name, 
        type: c.type 
    }));
    
    // 检查表是否存在
    const tableExistsResult = await db.select(
        "SELECT name FROM sqlite_master WHERE type='table' AND name=$1", 
        [tableName]
    );

    if (tableExistsResult.length === 0) {
        // 表不存在，直接创建
        const createSql = generateCreateSql(tableDef);
        await db.execute(createSql);
        warn(`[Schema] 表 '${tableName}' 不存在，已创建`);
        return;
    }

    // 表存在，获取现有列信息 (PRAGMA table_info)
    const existingColsResult = await db.select(`PRAGMA table_info(${tableName})`);
    const existingColumns = existingColsResult.map(col => ({
        name: col.name,
        type: col.type.toUpperCase() // 将类型统一转为大写进行比较
    }));
    
    // 比较 Schema
    const existingColMap = new Map(existingColumns.map(c => [c.name, c.type]));
    let shouldRebuild = false;
    let columnsToAdd = [];
    
    // 检查缺少的列和类型不匹配的列
    for (const expectedCol of expectedColumns) {
        const existingType = existingColMap.get(expectedCol.name);

        if (!existingType) {
            // 列缺失：可以简单添加（使用 ALTER TABLE）
            columnsToAdd.push(expectedCol);
        } else if (existingType !== expectedCol.type) {
            // 列类型不匹配：必须重建表
            warn(`[Schema] 表 '${tableName}' 的列 '${expectedCol.name}' 类型不匹配 (期望: ${expectedCol.type}, 现有: ${existingType})。将重建表`);
            shouldRebuild = true;
            break; // 只要有一个列需要重建，就退出循环
        }
    }
    
    // 检查多余的列（需要重建表）
    if (!shouldRebuild) {
        for (const existingCol of existingColumns) {
            if (!expectedColumns.some(e => e.name === existingCol.name)) {
                warn(`[Schema] 表 '${tableName}' 存在多余列 '${existingCol.name}'。将重建表`);
                shouldRebuild = true;
                break;
            }
        }
    }

    // 执行 Schema 修正
    if (shouldRebuild) {
        await rebuildTable(tableName, tableDef);
        info(`[Schema] 表 '${tableName}' 已重建以同步结构`);
    } else if (columnsToAdd.length > 0) {
        // 只添加缺少的列 (ALTER TABLE ADD COLUMN)
        for (const col of columnsToAdd) {
            const def = tableDef.columns.find(c => c.name === col.name);
            let alterSql = `ALTER TABLE ${tableName} ADD COLUMN ${def.name} ${def.type}`;
            if (def.defaultValue !== undefined) {
                 alterSql += ` DEFAULT (${def.defaultValue})`;
            }
            // 注意：SQLite 不允许在 ADD COLUMN 时设置 NOT NULL (除非表为空)
            if (def.notNull && def.defaultValue === undefined) {
                warn(`[Schema] 警告: 无法为表 '${tableName}' 添加非空列 '${def.name}'，请手动处理或重建`);
            } else if (def.notNull && def.defaultValue !== undefined) {
                // 如果有默认值，并且是 NOT NULL，则可以添加
                alterSql = `ALTER TABLE ${tableName} ADD COLUMN ${def.name} ${def.type} NOT NULL DEFAULT (${def.defaultValue})`;
            }
            
            await db.execute(alterSql);
            info(`[Schema] 表 '${tableName}' 已添加列 '${col.name}'`);
        }
    }
    // 如果 columnsToAdd.length === 0 且 shouldRebuild === false，则结构一致，无需操作
}


/**
 * 核心的重建表逻辑 (DROP/CREATE/COPY 模式)
 * 仅保留期望的列并将数据复制过去。
 * @param {string} tableName - 表名
 * @param {object} tableDef - 期望的表结构定义
 */
async function rebuildTable(tableName, tableDef) {
    const TEMP_TABLE_NAME = `${tableName}_temp_rebuild`;
    
    // 获取期望的列名列表（用于 INSERT INTO）
    const expectedColNames = tableDef.columns.map(c => c.name);
    const existingColsResult = await db.select(`PRAGMA table_info(${tableName})`);
    const existingColNames = existingColsResult.map(c => c.name);
    
    // 确定可以被复制的列（新旧表中都存在的列）
    const commonCols = expectedColNames.filter(name => existingColNames.includes(name));
    
    // 创建临时表 (使用期望的 Schema)
    const createTempSql = generateCreateSql({ ...tableDef, name: TEMP_TABLE_NAME });
    await db.execute(createTempSql);
    
    // 复制数据 (只复制共同的列)
    const copySql = `INSERT INTO ${TEMP_TABLE_NAME} (${commonCols.join(', ')}) 
                     SELECT ${commonCols.join(', ')} FROM ${tableName}`;
    await db.execute(copySql);
    
    // 删除旧表
    await db.execute(`DROP TABLE ${tableName}`);
    
    // 重命名临时表为正式表
    await db.execute(`ALTER TABLE ${TEMP_TABLE_NAME} RENAME TO ${tableName}`);
}

function disConnect(){
    return new Promise(async (resolve, reject) => {
        try {
            await db.close();

            db = null;
            isConnect = false;
            resolve();
        } catch (error) {
            reject(error);
        }
    })
}

// 处理条件中的?
function placeholderEscape(whereClause, valuesLength){
    let processedWhereClause = whereClause;
    let sql = "";
    if (whereClause) {
        const wherePlaceholders = whereClause.match(/\?/g);
        if (wherePlaceholders) {
            wherePlaceholders.forEach((_, index) => {
                processedWhereClause = processedWhereClause.replace('?', `\$${valuesLength + index + 1}`);
            });
        }
        sql += ` WHERE ${processedWhereClause}`;
    }

    return sql;
}

/**
 * 插入数据的通用函数
 * @param {string} tableName - 表名
 * @param {string[]} columns - 列名数组
 * @param {any[]} values - 值数组
 * @example insert("fingerprint", ["fingerprint", "last_ip", "last_time"], ["1", "2", "3"]).then((data)=>console.log(data)).catch();
 */
function insert(tableName, columns, values){
    return new Promise(async (resolve, reject) => {
        // 动态生成列名和占位符
        const columnNames = columns.join(', ');
        // 占位符为 $1, $2, $3...
        const placeholders = columns.map((_, index) => `$${index + 1}`).join(', ');
        
        const sql = `INSERT INTO ${tableName} (${columnNames}) VALUES (${placeholders})`;

        try {
            const result = await db.execute(sql, values);
            resolve({
                sql,
                changes: result.rowsAffected,
                lastId: result.lastInsertId
            })
        } catch (error) {
            reject(error);
        }
    })
}

/**
 * 更新数据的通用函数
 * @param {string} tableName - 表名
 * @param {object} data - 要更新的列和值的对象
 * @param {string} whereClause - 可选的 WHERE 条件
 * @param {any[]} whereArgs - 可选的 WHERE 条件参数数组
 * @example update("fingerprint", {fingerprint: "4", last_ip: "5", last_time: "6"}, "id = ?", [4]).then((data)=>console.log(data)).catch((error)=>console.log(error));
 */
function update(tableName, data, whereClause = '', whereArgs = []) {
    return new Promise(async (resolve, reject) => {
        // 提取列和值
        const columns = Object.keys(data);
        const values = Object.values(data);
        // 动态生成 SET 子句，使用 $1, $2, ... 占位符
        const setClause = columns.map((col, index) => `${col} = $${index + 1}`).join(', ');
        let sql = `UPDATE ${tableName} SET ${setClause}`;
        // 处理 WHERE 子句中的占位符
        sql += placeholderEscape(whereClause, values.length);
        // 合并更新的值和 WHERE 子句的参数
        const allValues = [...values, ...whereArgs];
        // 执行 SQL
        try {
            const result = await db.execute(sql, allValues);
            resolve({
                sql,
                changes: result.rowsAffected
            })
        } catch (error) {
            reject(error);
        }
    });
}

/**
 * 通用查询函数
 * @param {string} tableName - 表名
 * @param {string[]} columns - 要查询的列名数组，传 `*` 查询所有列
 * @param {string} whereClause - 可选的 WHERE 条件
 * @param {any[]} whereArgs - 可选的 WHERE 条件参数数组
 * @example select("fingerprint", ['*'], 'id = ? OR id = ?', [1,2]).then((data)=>console.log(data)).catch((error)=>console.log(error));
 */
function select(tableName, columns, whereClause = '', whereArgs = []) {
    return new Promise(async (resolve, reject) => {
        // 动态生成查询语句
        const columnList = columns.length > 0 ? columns.join(', ') : '*';
        let sql = `SELECT ${columnList} FROM ${tableName}`;
        if(whereClause){
            sql += placeholderEscape(whereClause, 0);
        }
        
        // 执行查询
        try {
            const result = await db.select(sql, whereArgs);
            resolve({
                sql,
                rows: result
            });
        } catch (error) {
            reject(error);
        }
    });
}

/**
 * 通用删除函数
 * @param {string} tableName - 表名
 * @param {string} whereClause - 可选的 WHERE 条件
 * @param {any[]} whereArgs - 可选的 WHERE 条件参数数组
 * @example deleteData("fingerprint", "id = ?", [1]).then((data)=>console.log(data)).catch((error)=>console.log(error));
 */
function deleteData(tableName, whereClause = '', whereArgs = []) {
    return new Promise(async (resolve, reject) => {
        // 动态生成删除语句
        let sql = `DELETE FROM ${tableName}`;
        if (whereClause) {
            sql += placeholderEscape(whereClause, 0);
        }

        // 执行删除操作
        try {
            const result = await db.execute(sql, whereArgs);
            resolve({
                sql,
                changes: result.rowsAffected
            });
        } catch (error) {
            reject(error);
        }
    });
}

/**
 * 自定义查询函数,自己写SQL和条件
 * @param {string} sql - SQL语句
 * @param {any[]} whereArgs - 可选的 WHERE 条件参数数组
 */
function selectCustom(sql, whereArgs) {
    return new Promise(async (resolve, reject) => {
        // 执行查询
        try {
            const result = await db.select(sql, whereArgs);
            resolve({
                sql,
                rows: result
            });
        } catch (error) {
            reject(error);
        }
    });
}

export {
    connect, disConnect, insert, update, select, deleteData, selectCustom
}