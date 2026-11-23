import { invoke } from "@tauri-apps/api/core";

export interface AppInfo { name:string;version:string }
export interface Game { app_id:number;name:string;install_dir:string;library_path:string;prefix:string|null;proton_version:string|null }
export interface Trainer { id:number;name:string;exe_path:string;file_size:number }
export interface RunRecord { id:number;app_id:number;trainer_id:number|null;started_at:string;finished_at:string|null;status:string;exit_code:number|null;error:string|null }
export function isTauri():boolean{return typeof window!=="undefined"&&"__TAURI_INTERNALS__" in window}
export const getAppInfo=()=>invoke<AppInfo>("get_app_info");
export const listGames=()=>invoke<Game[]>("list_games_command");
export const listTrainers=(appId?:number)=>invoke<Trainer[]>("list_trainers_command",{appId:appId??null});
export const importTrainer=(path:string,appId:number)=>invoke<Trainer>("import_trainer_command",{path,appId});
export const removeTrainer=(trainerId:number)=>invoke<void>("remove_trainer_command",{trainerId});
export const launchTrainer=(appId:number,trainerId:number)=>invoke<{success:boolean;exit_code:number|null;message:string}>("launch_trainer_command",{appId,trainerId});
export const getHistory=()=>invoke<RunRecord[]>("history_command");
export const getSteamRoot=()=>invoke<string|null>("get_steam_root_command");
export const setSteamRoot=(path:string)=>invoke<void>("set_steam_root_command",{path});
