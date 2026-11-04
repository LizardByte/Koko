/** Presents themed confirmation dialogs with native focus trapping and keyboard dismissal. */
import './confirmationDialog.css';

/** Text and destructive-action label for a confirmation presented inside Koko. */
export interface ConfirmationOptions {title:string;message:string;confirmLabel:string}

/** Resolve once, restoring focus on dismissal and cancelling if browser navigation leaves the page. */
export function confirmAction(options:ConfirmationOptions):Promise<boolean> {
  const previous=document.activeElement as HTMLElement|null;
  const dialog=document.createElement('dialog');dialog.className='confirmation-dialog';
  dialog.setAttribute('aria-labelledby','confirmation-dialog-title');
  dialog.setAttribute('aria-describedby','confirmation-dialog-message');
  const title=document.createElement('h2');title.id='confirmation-dialog-title';title.textContent=options.title;
  const message=document.createElement('p');message.id='confirmation-dialog-message';message.textContent=options.message;
  const actions=document.createElement('div');actions.className='confirmation-dialog-actions';
  const cancel=document.createElement('button');cancel.type='button';cancel.textContent='Cancel';
  const confirm=document.createElement('button');confirm.type='button';confirm.className='confirmation-dialog-confirm';confirm.textContent=options.confirmLabel;
  actions.append(cancel,confirm);dialog.append(title,message,actions);
  return new Promise(resolve=>{
    const events=new AbortController();let finished=false;
    const finish=(accepted:boolean)=>{
      if(finished) {return;}finished=true;events.abort();dialog.close();dialog.remove();
      if(previous?.isConnected) {previous.focus();}resolve(accepted);
    };
    cancel.addEventListener('click',()=>finish(false),{signal:events.signal});
    confirm.addEventListener('click',()=>finish(true),{signal:events.signal});
    dialog.addEventListener('cancel',event=>{event.preventDefault();finish(false);},{signal:events.signal});
    dialog.addEventListener('close',()=>finish(false),{signal:events.signal});
    dialog.addEventListener('click',event=>{
      const box=dialog.getBoundingClientRect();
      if(event.target===dialog&&(event.clientX<box.left||event.clientX>box.right||event.clientY<box.top||event.clientY>box.bottom)) finish(false);
    },{signal:events.signal});
    globalThis.addEventListener('popstate',()=>finish(false),{signal:events.signal});
    document.body.append(dialog);dialog.showModal();cancel.focus();
  });
}
